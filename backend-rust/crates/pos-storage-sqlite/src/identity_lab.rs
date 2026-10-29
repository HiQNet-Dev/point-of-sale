//! Adaptador SQLite **de laboratorio** para identidad, sesión y permisos.
//!
//! # Alcance
//!
//! Este módulo demuestra la frontera DB-authoritative sobre SQLite real, pero
//! **no es el esquema Local de producto**: las tablas `identity_*_lab` se crean
//! solo para pruebas y no viven en `migrations/local`. El DDL de `pos_user`,
//! `local_credential`, `local_session` y RBAC sigue pendiente de aprobación
//! normativa.
//!
//! El adaptador trabaja sobre una conexión concreta, de modo que la misma
//! verificación de identidad, sesión, membership, estado de sucursal y permisos
//! puede ejecutarse dentro de la unidad de trabajo (`BEGIN IMMEDIATE` + RAII) de
//! la mutación, sin cachés ni decisiones obsoletas.

use std::collections::BTreeSet;

use pos_core::{
    identity::{
        IdentityError, LocalIdentityRepository, LocalSession, NormalizedEmail, PermissionAuthority,
        PermissionCode, PosUser, PosUserStatus, SessionHash, UtcSeconds,
    },
    organization_branch::{DisplayName, EntityId},
};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::CasOutcome;

/// Sentencias del esquema de laboratorio; nunca se ejecutan en producto.
pub const LAB_SCHEMA_STATEMENTS: [&str; 6] = [
    "CREATE TABLE IF NOT EXISTS identity_user_lab (\
        id TEXT PRIMARY KEY NOT NULL, \
        organization_id TEXT NOT NULL REFERENCES organization(id), \
        email TEXT NOT NULL, \
        display_name TEXT NOT NULL, \
        status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'INACTIVE')), \
        version INTEGER NOT NULL CHECK (version >= 0), \
        UNIQUE (id, organization_id), \
        UNIQUE (organization_id, email)\
     ) STRICT",
    "CREATE TABLE IF NOT EXISTS identity_session_lab (\
        id TEXT PRIMARY KEY NOT NULL, \
        user_id TEXT NOT NULL, \
        session_hash TEXT NOT NULL UNIQUE, \
        created_at INTEGER NOT NULL, \
        expires_at INTEGER NOT NULL, \
        revoked_at INTEGER, \
        session_version INTEGER NOT NULL CHECK (session_version >= 1), \
        CHECK (expires_at > created_at), \
        FOREIGN KEY (user_id) REFERENCES identity_user_lab(id)\
     ) STRICT",
    "CREATE TABLE IF NOT EXISTS identity_role_lab (\
        id TEXT PRIMARY KEY NOT NULL, \
        organization_id TEXT NOT NULL REFERENCES organization(id), \
        code TEXT NOT NULL, \
        UNIQUE (id, organization_id)\
     ) STRICT",
    "CREATE TABLE IF NOT EXISTS identity_role_permission_lab (\
        role_id TEXT NOT NULL REFERENCES identity_role_lab(id), \
        permission_code TEXT NOT NULL, \
        PRIMARY KEY (role_id, permission_code)\
     ) STRICT",
    "CREATE TABLE IF NOT EXISTS identity_user_role_lab (\
        organization_id TEXT NOT NULL, \
        user_id TEXT NOT NULL, \
        role_id TEXT NOT NULL, \
        PRIMARY KEY (organization_id, user_id, role_id), \
        FOREIGN KEY (user_id, organization_id) REFERENCES identity_user_lab(id, organization_id), \
        FOREIGN KEY (role_id, organization_id) REFERENCES identity_role_lab(id, organization_id)\
     ) STRICT",
    "CREATE TABLE IF NOT EXISTS identity_membership_lab (\
        id TEXT PRIMARY KEY NOT NULL, \
        organization_id TEXT NOT NULL, \
        branch_id TEXT NOT NULL, \
        user_id TEXT NOT NULL, \
        status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'INACTIVE')), \
        UNIQUE (organization_id, branch_id, user_id), \
        UNIQUE (id, organization_id), \
        FOREIGN KEY (branch_id, organization_id) REFERENCES branch(id, organization_id), \
        FOREIGN KEY (user_id, organization_id) REFERENCES identity_user_lab(id, organization_id)\
     ) STRICT",
];

/// Segunda tanda del esquema de laboratorio (roles de membership + índices).
pub const LAB_SCHEMA_TAIL_STATEMENTS: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS identity_membership_role_lab (\
        organization_id TEXT NOT NULL, \
        membership_id TEXT NOT NULL, \
        role_id TEXT NOT NULL, \
        PRIMARY KEY (organization_id, membership_id, role_id), \
        FOREIGN KEY (membership_id, organization_id) REFERENCES identity_membership_lab(id, organization_id), \
        FOREIGN KEY (role_id, organization_id) REFERENCES identity_role_lab(id, organization_id)\
     ) STRICT",
    "CREATE INDEX IF NOT EXISTS idx_identity_session_lab_hash ON identity_session_lab (session_hash)",
];

/// Crea el esquema de laboratorio sobre una base ya migrada con `0001`.
///
/// Es idempotente y exclusivamente para pruebas; no se invoca desde `pos-local`.
pub async fn ensure_lab_identity_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for statement in LAB_SCHEMA_STATEMENTS {
        sqlx::query(statement).execute(pool).await?;
    }
    for statement in LAB_SCHEMA_TAIL_STATEMENTS {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

/// Adaptador de identidad y permisos ligado a una conexión SQLite concreta.
pub struct SqliteIdentityLab<'a> {
    connection: &'a mut SqliteConnection,
}

impl<'a> SqliteIdentityLab<'a> {
    /// Enlaza el adaptador a una conexión; puede ser la de una UoW activa.
    pub fn new(connection: &'a mut SqliteConnection) -> Self {
        Self { connection }
    }

    /// Busca una sesión por hash y deriva su organización desde `pos_user`.
    pub async fn session_by_hash(
        &mut self,
        hash: &SessionHash,
    ) -> Result<Option<LocalSession>, IdentityError> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT s.id, s.user_id, u.organization_id, s.session_hash, s.created_at, s.expires_at, s.revoked_at, s.session_version \
             FROM identity_session_lab s \
             JOIN identity_user_lab u ON u.id = s.user_id \
             WHERE s.session_hash = ?",
        )
        .bind(hash.as_storage_str())
        .fetch_optional(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        row.map(SessionRow::into_domain).transpose()
    }

    /// Busca un usuario dentro de su organización.
    pub async fn user_by_id(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, IdentityError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, organization_id, email, display_name, status, version \
             FROM identity_user_lab WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_optional(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        row.map(UserRow::into_domain).transpose()
    }

    /// Resuelve permisos para organización o para una sucursal activa.
    ///
    /// Un scope de sucursal exige sucursal activa, pertenencia a la organización
    /// y membership activa; de lo contrario falla cerrado con
    /// [`IdentityError::ScopeDenied`].
    pub async fn effective_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        let Some(branch_id) = branch_id else {
            return self
                .organization_permissions(user_id, organization_id)
                .await;
        };
        let branch_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM branch WHERE organization_id = ? AND id = ?",
        )
        .bind(organization_id.as_str())
        .bind(branch_id.as_str())
        .fetch_optional(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?
        .ok_or(IdentityError::ScopeDenied)?;
        if branch_status != "ACTIVE" {
            return Err(IdentityError::ScopeDenied);
        }
        let membership = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM identity_membership_lab \
             WHERE organization_id = ? AND branch_id = ? AND user_id = ? AND status = 'ACTIVE'",
        )
        .bind(organization_id.as_str())
        .bind(branch_id.as_str())
        .bind(user_id.as_str())
        .fetch_one(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        if membership == 0 {
            return Err(IdentityError::ScopeDenied);
        }
        let codes = sqlx::query_scalar::<_, String>(
            "SELECT permission_code FROM (\
                SELECT rp.permission_code FROM identity_user_role_lab ur \
                    JOIN identity_role_permission_lab rp ON rp.role_id = ur.role_id \
                    WHERE ur.organization_id = ? AND ur.user_id = ? \
                UNION \
                SELECT rp.permission_code FROM identity_membership_lab m \
                    JOIN identity_membership_role_lab mr \
                        ON mr.membership_id = m.id AND mr.organization_id = m.organization_id \
                    JOIN identity_role_permission_lab rp ON rp.role_id = mr.role_id \
                    WHERE m.organization_id = ? AND m.branch_id = ? AND m.user_id = ? AND m.status = 'ACTIVE'\
             ) ORDER BY permission_code",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .bind(organization_id.as_str())
        .bind(branch_id.as_str())
        .bind(user_id.as_str())
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        codes.into_iter().map(parse_permission).collect()
    }

    async fn organization_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        let codes = sqlx::query_scalar::<_, String>(
            "SELECT rp.permission_code FROM identity_user_role_lab ur \
             JOIN identity_role_permission_lab rp ON rp.role_id = ur.role_id \
             WHERE ur.organization_id = ? AND ur.user_id = ? \
             ORDER BY rp.permission_code",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        codes.into_iter().map(parse_permission).collect()
    }

    /// Revoca una sesión con CAS sobre `session_version`, sobre la misma conexión.
    ///
    /// Devuelve [`CasOutcome::Missing`] si la sesión no existe y
    /// [`CasOutcome::Conflict`] si la versión no coincide o ya estaba revocada.
    /// No distingue organizaciones: el llamador autorizado ya fijó el scope.
    pub async fn revoke_session_cas(
        &mut self,
        session_id: &EntityId,
        expected_version: u64,
        revoked_at: i64,
    ) -> Result<CasOutcome, IdentityError> {
        let expected =
            i64::try_from(expected_version).map_err(|_| IdentityError::StorageUnavailable)?;
        let result = sqlx::query(
            "UPDATE identity_session_lab SET revoked_at = ?, session_version = session_version + 1 \
             WHERE id = ? AND session_version = ? AND revoked_at IS NULL",
        )
        .bind(revoked_at)
        .bind(session_id.as_str())
        .bind(expected)
        .execute(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM identity_session_lab WHERE id = ?")
                .bind(session_id.as_str())
                .fetch_one(&mut *self.connection)
                .await
                .map_err(|_| IdentityError::StorageUnavailable)?;
        Ok(if exists == 0 {
            CasOutcome::Missing
        } else {
            CasOutcome::Conflict
        })
    }
}

impl LocalIdentityRepository for SqliteIdentityLab<'_> {
    async fn find_session_by_hash(
        &mut self,
        hash: &SessionHash,
    ) -> Result<Option<LocalSession>, IdentityError> {
        self.session_by_hash(hash).await
    }
    async fn find_user(
        &mut self,
        organization_id: &EntityId,
        user_id: &EntityId,
    ) -> Result<Option<PosUser>, IdentityError> {
        self.user_by_id(organization_id, user_id).await
    }
}

impl PermissionAuthority for SqliteIdentityLab<'_> {
    async fn resolve_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        self.effective_permissions(user_id, organization_id, branch_id)
            .await
    }
}

fn parse_permission(code: String) -> Result<PermissionCode, IdentityError> {
    PermissionCode::new(&code).map_err(|_| IdentityError::StorageUnavailable)
}

#[derive(FromRow)]
struct SessionRow {
    id: String,
    user_id: String,
    organization_id: String,
    session_hash: String,
    created_at: i64,
    expires_at: i64,
    revoked_at: Option<i64>,
    session_version: i64,
}
impl SessionRow {
    fn into_domain(self) -> Result<LocalSession, IdentityError> {
        LocalSession::restore(
            EntityId::parse(&self.id).map_err(|_| IdentityError::StorageUnavailable)?,
            EntityId::parse(&self.user_id).map_err(|_| IdentityError::StorageUnavailable)?,
            EntityId::parse(&self.organization_id)
                .map_err(|_| IdentityError::StorageUnavailable)?,
            SessionHash::new(&self.session_hash)?,
            UtcSeconds::new(self.created_at)?,
            UtcSeconds::new(self.expires_at)?,
            self.revoked_at.map(UtcSeconds::new).transpose()?,
            u64::try_from(self.session_version).map_err(|_| IdentityError::StorageUnavailable)?,
        )
    }
}

#[derive(FromRow)]
struct UserRow {
    id: String,
    organization_id: String,
    email: String,
    display_name: String,
    status: String,
    version: i64,
}
impl UserRow {
    fn into_domain(self) -> Result<PosUser, IdentityError> {
        let status = match self.status.as_str() {
            "ACTIVE" => PosUserStatus::Active,
            "INACTIVE" => PosUserStatus::Inactive,
            _ => return Err(IdentityError::StorageUnavailable),
        };
        Ok(PosUser::restore(
            EntityId::parse(&self.id).map_err(|_| IdentityError::StorageUnavailable)?,
            EntityId::parse(&self.organization_id)
                .map_err(|_| IdentityError::StorageUnavailable)?,
            DisplayName::new(&self.display_name).map_err(|_| IdentityError::StorageUnavailable)?,
            NormalizedEmail::new(&self.email)?,
            status,
            u64::try_from(self.version).map_err(|_| IdentityError::StorageUnavailable)?,
        ))
    }
}
