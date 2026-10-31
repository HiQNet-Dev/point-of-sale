//! Observación del historial de migraciones y migrador autorizado.
//!
//! Solo lee `_sqlx_migrations`, la existencia del archivo y aplica el root
//! aprobado embebido. No interpreta tablas de negocio ni conoce el esquema
//! candidato.

use pos_application::installation::{
    AuthorizedMigrator, InstallationError, InstallationReader, SchemaObservation,
};
use sqlx::SqlitePool;

/// Adaptador del ciclo de vida de instalación sobre un pool ya abierto.
///
/// Implementa los dos puertos que consume la aplicación: lectura del historial y
/// aplicación del migrador aprobado. `database_existed` procede del composition
/// root, que es el único punto que conoce la ruta.
pub struct SqliteInstallationSchema {
    pool: SqlitePool,
    database_existed: bool,
}

impl SqliteInstallationSchema {
    /// Construye el adaptador sin abrir conexiones adicionales.
    pub fn new(pool: SqlitePool, database_existed: bool) -> Self {
        Self {
            pool,
            database_existed,
        }
    }
}

impl InstallationReader for SqliteInstallationSchema {
    async fn observe(&self) -> Result<SchemaObservation, InstallationError> {
        let history_present: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| InstallationError::Unavailable)?;
        if history_present == 0 {
            return Ok(SchemaObservation {
                database_existed: self.database_existed,
                applied_migrations: 0,
                latest_version: None,
            });
        }
        let (applied, latest): (i64, Option<i64>) =
            sqlx::query_as("SELECT COUNT(*), MAX(version) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&self.pool)
                .await
                .map_err(|_| InstallationError::Unavailable)?;
        let applied = u32::try_from(applied).map_err(|_| InstallationError::Unavailable)?;
        Ok(SchemaObservation {
            database_existed: self.database_existed,
            applied_migrations: applied,
            latest_version: latest,
        })
    }
}

impl AuthorizedMigrator for SqliteInstallationSchema {
    async fn apply_approved_migrations(&self) -> Result<(), InstallationError> {
        crate::connection::migrate(&self.pool)
            .await
            .map_err(|_| InstallationError::MigrationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pos_application::installation::{ApprovedSchema, prepare_installation};
    use sqlx::SqlitePool;
    use std::{path::Path, time::Duration};

    async fn pool(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
        Ok(crate::open(path, Duration::from_secs(5)).await?)
    }

    fn approved() -> Result<ApprovedSchema, pos_application::installation::InstallationError> {
        ApprovedSchema::new(crate::approved_schema_version())
    }

    #[tokio::test]
    async fn empty_file_has_no_history() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("installation.sqlite");
        let pool = pool(&database).await?;
        let observation = SqliteInstallationSchema::new(pool.clone(), false)
            .observe()
            .await?;
        assert_eq!(
            observation,
            SchemaObservation {
                database_existed: false,
                applied_migrations: 0,
                latest_version: None,
            }
        );
        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn preparation_migrates_then_reports_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("installation.sqlite");

        let first_pool = pool(&database).await?;
        let first = SqliteInstallationSchema::new(first_pool.clone(), database.exists());
        let first_approved = approved()?;
        let first_report = prepare_installation(&first, &first, first_approved).await?;
        assert_eq!(
            first_report.schema_version,
            crate::approved_schema_version()
        );
        assert_eq!(first_report.applied_migrations, 17);
        assert!(first_report.was_migrated());
        first_pool.close().await;

        // Reapertura del mismo archivo: no vuelve a migrar y conserva el historial.
        let second_pool = pool(&database).await?;
        let second = SqliteInstallationSchema::new(second_pool.clone(), database.exists());
        let second_report = prepare_installation(&second, &second, first_approved).await?;
        assert_eq!(second_report.schema_version, first_report.schema_version);
        assert!(!second_report.was_migrated());
        assert!(second_report.database_existed);
        second_pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn reopen_rejects_a_changed_checksum() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let pool = pool(&directory.path().join("installation.sqlite")).await?;
        crate::migrate(&pool).await?;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = x'00' WHERE version = 1")
            .execute(&pool)
            .await?;
        let schema = SqliteInstallationSchema::new(pool.clone(), true);
        assert_eq!(
            prepare_installation(&schema, &schema, approved()?).await,
            Err(InstallationError::MigrationFailed)
        );
        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn reopen_rejects_a_failed_migration() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let pool = pool(&directory.path().join("installation.sqlite")).await?;
        crate::migrate(&pool).await?;
        sqlx::query("UPDATE _sqlx_migrations SET success = 0 WHERE version = 1")
            .execute(&pool)
            .await?;
        let schema = SqliteInstallationSchema::new(pool.clone(), true);
        assert!(
            prepare_installation(&schema, &schema, approved()?)
                .await
                .is_err()
        );
        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn approved_version_follows_the_embedded_migrator() {
        assert_eq!(crate::approved_schema_version(), 17);
    }
}
