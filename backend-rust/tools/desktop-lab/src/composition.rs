//! Composition root exclusivo del laboratorio; no activa pos-local ni DDL productivo.
use std::{error::Error, time::Duration};

use pos_application::{catalog::CatalogPage, organization::list_authorized_branches};
use pos_core::identity::VerifiedPrincipal;
use pos_storage_sqlite::{
    SqliteOrganizationBranchStore, SqliteUnitOfWork, catalog::SqliteCatalogApplication,
};
use sqlx::SqlitePool;
use tempfile::TempDir;

use crate::dto::{BranchDto, CategoryDto, IpcError, PageRequest, ProductDto};

/// Recursos privados del escenario. No acepta rutas ni sesiones desde JavaScript.
pub struct DesktopState {
    pool: SqlitePool,
    _directory: TempDir,
}

impl DesktopState {
    /// Construye una instalación sintética efímera con los adaptadores existentes.
    pub async fn synthetic() -> Result<Self, Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let pool = pos_storage_sqlite::open(
            &directory.path().join("desktop.sqlite"),
            Duration::from_secs(5),
        )
        .await?;
        pos_lab::run(&pool).await?;
        Ok(Self {
            pool,
            _directory: directory,
        })
    }

    /// Cierra las conexiones antes de liberar el directorio temporal al salir.
    pub async fn shutdown(&self) {
        self.pool.close().await;
    }

    async fn principal(&self) -> Result<VerifiedPrincipal, IpcError> {
        // La sesión es una fixture declarada. Se consulta de nuevo en cada llamada:
        // revocación, estados y permisos siguen bajo autoridad de SQLite.
        pos_lab::authenticate_candidate_session(&self.pool, None)
            .await
            .map_err(|_| IpcError::Unauthorized)
    }

    /// Ejecuta la consulta Application; no filtra mediante JavaScript.
    pub async fn branches(&self) -> Result<Vec<BranchDto>, IpcError> {
        let principal = self.principal().await?;
        let reader = SqliteOrganizationBranchStore::new(self.pool.clone());
        let branches = list_authorized_branches(&reader, &principal).await?;
        Ok(branches
            .into_iter()
            .map(|branch| BranchDto {
                id: branch.id().as_str().into(),
                code: branch.code().as_str().into(),
                name: branch.name().as_str().into(),
                status: branch.status().as_str().into(),
            })
            .collect())
    }

    /// Lista categorías scoped usando el caso de uso existente.
    pub async fn categories(&self, page: PageRequest) -> Result<Vec<CategoryDto>, IpcError> {
        let principal = self.principal().await?;
        let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(self.pool.clone()));
        let categories = catalog
            .list_categories(
                &principal,
                None,
                CatalogPage {
                    limit: page.limit,
                    offset: page.offset,
                },
            )
            .await?;
        Ok(categories
            .into_iter()
            .map(|category| CategoryDto {
                id: category.id().as_str().into(),
                code: category.code().as_str().into(),
                name: category.name().as_str().into(),
                version: category.revision().to_string(),
            })
            .collect())
    }

    /// Lista productos sin convertir sus precios a float ni a unidades menores.
    pub async fn products(&self, page: PageRequest) -> Result<Vec<ProductDto>, IpcError> {
        let principal = self.principal().await?;
        let catalog = SqliteCatalogApplication::new(SqliteUnitOfWork::new(self.pool.clone()));
        let products = catalog
            .list_products(
                &principal,
                None,
                None,
                CatalogPage {
                    limit: page.limit,
                    offset: page.offset,
                },
            )
            .await?;
        Ok(products
            .into_iter()
            .map(|product| ProductDto {
                id: product.id().as_str().into(),
                sku: product.sku().as_str().into(),
                name: product.name().as_str().into(),
                unit_price: product.unit_price().to_string(),
                category_id: product.category_id().map(|id| id.as_str().into()),
                version: product.revision().to_string(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_principal_cannot_enumerate_the_organization() -> Result<(), Box<dyn Error>> {
        tauri::async_runtime::block_on(async {
            let state = DesktopState::synthetic().await?;
            let visible = state.branches().await?;
            let branch = pos_core::organization_branch::EntityId::parse(&visible[0].id)?;
            let principal =
                pos_lab::authenticate_candidate_session(&state.pool, Some(&branch)).await?;
            let reader = SqliteOrganizationBranchStore::new(state.pool.clone());
            assert!(matches!(
                list_authorized_branches(&reader, &principal).await,
                Err(pos_core::organization_branch::ApplicationError::Forbidden)
            ));
            state.pool.close().await;
            Ok(())
        })
    }

    #[test]
    fn permission_removal_is_effective_on_next_call() -> Result<(), Box<dyn Error>> {
        tauri::async_runtime::block_on(async {
            let state = DesktopState::synthetic().await?;
            sqlx::query("DELETE FROM role_permission WHERE permission_code = 'product.read'")
                .execute(&state.pool)
                .await?;
            assert!(matches!(
                state
                    .products(PageRequest {
                        limit: 10,
                        offset: 0
                    })
                    .await,
                Err(IpcError::Forbidden)
            ));
            assert!(
                !state
                    .categories(PageRequest {
                        limit: 10,
                        offset: 0
                    })
                    .await?
                    .is_empty()
            );
            state.pool.close().await;
            Ok(())
        })
    }

    #[test]
    fn expired_session_is_not_authorized() -> Result<(), Box<dyn Error>> {
        tauri::async_runtime::block_on(async {
            let state = DesktopState::synthetic().await?;
            sqlx::query("UPDATE local_session SET created_at = '2026-10-30T08:00:00Z', expires_at = '2026-10-30T08:59:00Z'").execute(&state.pool).await?;
            assert!(matches!(
                state.branches().await,
                Err(IpcError::Unauthorized)
            ));
            state.pool.close().await;
            Ok(())
        })
    }

    #[test]
    fn branch_queries_never_return_other_organization() -> Result<(), Box<dyn Error>> {
        tauri::async_runtime::block_on(async {
            let state = DesktopState::synthetic().await?;
            sqlx::query("INSERT INTO organization SELECT '00000000-0000-0000-0000-000000000999', 'foreign', name, currency, timezone, locale, stock_negative_policy, status, version, created_at, updated_at FROM organization LIMIT 1")
                .execute(&state.pool).await?;
            sqlx::query("INSERT INTO branch SELECT '00000000-0000-0000-0000-000000000998', '00000000-0000-0000-0000-000000000999', 'foreign-branch', name, status, timezone_override, version, created_at, updated_at FROM branch LIMIT 1")
                .execute(&state.pool).await?;
            let branches = state.branches().await?;
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].code, "centro");
            state.pool.close().await;
            Ok(())
        })
    }

    #[test]
    fn authorization_is_revalidated_after_revocation() -> Result<(), Box<dyn Error>> {
        tauri::async_runtime::block_on(async {
            let state = DesktopState::synthetic().await?;
            assert!(
                !state
                    .products(PageRequest {
                        limit: 10,
                        offset: 0
                    })
                    .await?
                    .is_empty()
            );
            sqlx::query(
                "UPDATE local_session SET state = 'REVOKED', revoked_at = '2026-10-30T10:00:00Z'",
            )
            .execute(&state.pool)
            .await?;
            assert!(matches!(
                state
                    .products(PageRequest {
                        limit: 10,
                        offset: 0
                    })
                    .await,
                Err(IpcError::Unauthorized)
            ));
            state.pool.close().await;
            Ok(())
        })
    }
}
