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
        PermissionCode, PosUser, PosUserStatus, SessionHash, UtcSeconds, VerifiedPrincipal,
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
        status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')), \
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

/// Segunda tanda del esquema de laboratorio (roles de membership + índice).
pub const LAB_SCHEMA_TAIL_STATEMENTS: [&str; 3] = [
    "CREATE TABLE IF NOT EXISTS identity_membership_role_lab (\
        organization_id TEXT NOT NULL, \
        membership_id TEXT NOT NULL, \
        role_id TEXT NOT NULL, \
        PRIMARY KEY (organization_id, membership_id, role_id), \
        FOREIGN KEY (membership_id, organization_id) REFERENCES identity_membership_lab(id, organization_id), \
        FOREIGN KEY (role_id, organization_id) REFERENCES identity_role_lab(id, organization_id)\
     ) STRICT",
    "CREATE INDEX IF NOT EXISTS idx_identity_session_lab_hash ON identity_session_lab (session_hash)",
    "CREATE TABLE IF NOT EXISTS identity_credential_lab (\
        id TEXT PRIMARY KEY NOT NULL, \
        user_id TEXT NOT NULL, \
        organization_id TEXT NOT NULL, \
        credential_kind TEXT NOT NULL CHECK (credential_kind IN ('PASSWORD', 'PIN')), \
        algorithm TEXT NOT NULL CHECK (algorithm IN ('LEGACY_BCRYPT', 'ARGON2ID')), \
        secret_hash TEXT NOT NULL, \
        params_json TEXT NOT NULL CHECK (json_valid(params_json)), \
        policy_id TEXT NOT NULL, \
        state TEXT NOT NULL CHECK (state IN ('ACTIVE', 'RESET_REQUIRED', 'DISABLED')), \
        credential_version INTEGER NOT NULL CHECK (credential_version >= 1), \
        failed_login_attempts INTEGER NOT NULL CHECK (failed_login_attempts >= 0), \
        locked_until INTEGER, \
        created_at INTEGER NOT NULL, \
        updated_at INTEGER NOT NULL, \
        UNIQUE (user_id, credential_kind), \
        FOREIGN KEY (user_id, organization_id) REFERENCES identity_user_lab(id, organization_id)\
     ) STRICT",
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

    /// Resuelve permisos como Java `hasBranchPermission`.
    ///
    /// Incluye siempre las grants de organización (override) y añade la pierna
    /// de sucursal solo cuando la sucursal existe en la organización, está
    /// ACTIVA, la membership está ACTIVA y el rol ACTIVO. Una sucursal
    /// inexistente o ajena no produce error ni revela su existencia: no aporta
    /// permisos.
    pub async fn effective_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: Option<&EntityId>,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        let mut granted = self
            .organization_permissions(user_id, organization_id)
            .await?;
        if let Some(branch_id) = branch_id {
            granted.extend(
                self.branch_permissions(user_id, organization_id, branch_id, true)
                    .await?,
            );
        }
        Ok(granted)
    }

    /// Resuelve permisos como Java `hasBranchPermissionIgnoringBranchStatus`.
    ///
    /// Idéntico al scope de sucursal salvo que no exige `branch.status =
    /// 'ACTIVE'`; usuario, organización, membership y rol siguen siendo
    /// obligatoriamente ACTIVOS. Solo para flujos de recuperación explícitos.
    pub async fn effective_permissions_for_recovery(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        let mut granted = self
            .organization_permissions(user_id, organization_id)
            .await?;
        granted.extend(
            self.branch_permissions(user_id, organization_id, branch_id, false)
                .await?,
        );
        Ok(granted)
    }

    /// Grants de alcance organización: usuario, organización y rol ACTIVOS.
    async fn organization_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        let codes = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT rp.permission_code \
             FROM identity_user_role_lab ur \
             JOIN identity_user_lab u ON u.id = ur.user_id AND u.organization_id = ur.organization_id \
             JOIN identity_role_lab r ON r.id = ur.role_id AND r.organization_id = ur.organization_id \
             JOIN identity_role_permission_lab rp ON rp.role_id = r.id \
             JOIN organization o ON o.id = ur.organization_id \
             WHERE ur.organization_id = ? AND ur.user_id = ? \
               AND u.status = 'ACTIVE' AND r.status = 'ACTIVE' AND o.status = 'ACTIVE' \
             ORDER BY rp.permission_code",
        )
        .bind(organization_id.as_str())
        .bind(user_id.as_str())
        .fetch_all(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        codes.into_iter().map(parse_permission).collect()
    }

    /// Pierna de sucursal; `require_active_branch` distingue normal de recuperación.
    async fn branch_permissions(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: &EntityId,
        require_active_branch: bool,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        // Dos sentencias estáticas: el modo no se interpola en SQL dinámico.
        let codes = if require_active_branch {
            sqlx::query_scalar::<_, String>(BRANCH_PERMISSIONS_SQL)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(user_id.as_str())
                .fetch_all(&mut *self.connection)
                .await
        } else {
            sqlx::query_scalar::<_, String>(BRANCH_PERMISSIONS_RECOVERY_SQL)
                .bind(organization_id.as_str())
                .bind(branch_id.as_str())
                .bind(user_id.as_str())
                .fetch_all(&mut *self.connection)
                .await
        }
        .map_err(|_| IdentityError::StorageUnavailable)?;
        codes.into_iter().map(parse_permission).collect()
    }

    /// Revoca una sesión del principal autenticado con CAS sobre `session_version`.
    ///
    /// El scope de usuario y organización procede del principal, no de la
    /// petición: conocer un UUID ajeno no habilita la operación. Una sesión de
    /// otro usuario u organización responde [`CasOutcome::Missing`], igual que
    /// una inexistente, sin revelar recursos de otros tenants.
    pub async fn revoke_session_cas(
        &mut self,
        principal: &VerifiedPrincipal,
        session_id: &EntityId,
        expected_version: u64,
        revoked_at: i64,
    ) -> Result<CasOutcome, IdentityError> {
        let expected =
            i64::try_from(expected_version).map_err(|_| IdentityError::StorageUnavailable)?;
        let user_id = principal.user_id().as_str();
        let organization_id = principal.scope().organization_id().as_str();
        let result = sqlx::query(
            "UPDATE identity_session_lab SET revoked_at = ?, session_version = session_version + 1 \
             WHERE id = ? AND session_version = ? AND revoked_at IS NULL AND user_id = ? \
               AND EXISTS (SELECT 1 FROM identity_user_lab u \
                           WHERE u.id = identity_session_lab.user_id AND u.organization_id = ?)",
        )
        .bind(revoked_at)
        .bind(session_id.as_str())
        .bind(expected)
        .bind(user_id)
        .bind(organization_id)
        .execute(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        if result.rows_affected() == 1 {
            return Ok(CasOutcome::Updated);
        }
        let owned = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM identity_session_lab s \
             JOIN identity_user_lab u ON u.id = s.user_id \
             WHERE s.id = ? AND s.user_id = ? AND u.organization_id = ?",
        )
        .bind(session_id.as_str())
        .bind(user_id)
        .bind(organization_id)
        .fetch_one(&mut *self.connection)
        .await
        .map_err(|_| IdentityError::StorageUnavailable)?;
        Ok(if owned == 0 {
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
    async fn resolve_permissions_for_recovery(
        &mut self,
        user_id: &EntityId,
        organization_id: &EntityId,
        branch_id: &EntityId,
    ) -> Result<BTreeSet<PermissionCode>, IdentityError> {
        self.effective_permissions_for_recovery(user_id, organization_id, branch_id)
            .await
    }
}

/// Pierna de sucursal normal: exige sucursal ACTIVA.
const BRANCH_PERMISSIONS_SQL: &str = "SELECT DISTINCT rp.permission_code \
     FROM identity_membership_lab m \
     JOIN identity_user_lab u ON u.id = m.user_id AND u.organization_id = m.organization_id \
     JOIN branch b ON b.id = m.branch_id AND b.organization_id = m.organization_id \
     JOIN identity_membership_role_lab mr \
         ON mr.membership_id = m.id AND mr.organization_id = m.organization_id \
     JOIN identity_role_lab r ON r.id = mr.role_id AND r.organization_id = mr.organization_id \
     JOIN identity_role_permission_lab rp ON rp.role_id = r.id \
     JOIN organization o ON o.id = m.organization_id \
     WHERE m.organization_id = ? AND m.branch_id = ? AND m.user_id = ? \
       AND m.status = 'ACTIVE' AND b.status = 'ACTIVE' AND u.status = 'ACTIVE' \
       AND r.status = 'ACTIVE' AND o.status = 'ACTIVE' \
     ORDER BY rp.permission_code";

/// Pierna de sucursal de recuperación: no exige sucursal ACTIVA.
const BRANCH_PERMISSIONS_RECOVERY_SQL: &str = "SELECT DISTINCT rp.permission_code \
     FROM identity_membership_lab m \
     JOIN identity_user_lab u ON u.id = m.user_id AND u.organization_id = m.organization_id \
     JOIN branch b ON b.id = m.branch_id AND b.organization_id = m.organization_id \
     JOIN identity_membership_role_lab mr \
         ON mr.membership_id = m.id AND mr.organization_id = m.organization_id \
     JOIN identity_role_lab r ON r.id = mr.role_id AND r.organization_id = mr.organization_id \
     JOIN identity_role_permission_lab rp ON rp.role_id = r.id \
     JOIN organization o ON o.id = m.organization_id \
     WHERE m.organization_id = ? AND m.branch_id = ? AND m.user_id = ? \
       AND m.status = 'ACTIVE' AND u.status = 'ACTIVE' \
       AND r.status = 'ACTIVE' AND o.status = 'ACTIVE' \
     ORDER BY rp.permission_code";

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
