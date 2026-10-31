//! SQL por representación física; reglas de credencial y CAS compartidas.
pub(super) struct CredentialQueries {
    pub load: &'static str,
    pub active_user: &'static str,
    pub persist: &'static str,
    pub reset_state: &'static str,
}

pub(super) const LAB: CredentialQueries = CredentialQueries {
    load: "SELECT id,user_id,credential_kind,algorithm,secret_hash,params_json,policy_id,state,credential_version,failed_login_attempts,locked_until,created_at,updated_at FROM identity_credential_lab WHERE organization_id = ? AND user_id = ? AND credential_kind = ?",
    active_user: "SELECT status FROM identity_user_lab WHERE organization_id = ? AND id = ?",
    persist: "UPDATE identity_credential_lab SET algorithm = ?,secret_hash = ?,params_json = ?,policy_id = ?,state = ?,credential_version = ?,failed_login_attempts = ?,locked_until = ?,updated_at = ? WHERE organization_id = ? AND user_id = ? AND credential_kind = ? AND credential_version = ?",
    reset_state: "RESET_REQUIRED",
};

pub(super) const CANONICAL: CredentialQueries = CredentialQueries {
    load: "SELECT id,user_id,credential_kind,algorithm,secret_hash,params_json,policy_id,state,credential_version,failed_login_attempts,CAST(strftime('%s',locked_until) AS INTEGER) AS locked_until,CAST(strftime('%s',created_at) AS INTEGER) AS created_at,CAST(strftime('%s',updated_at) AS INTEGER) AS updated_at FROM local_credential WHERE organization_id = ? AND user_id = ? AND credential_kind = ?",
    active_user: "SELECT status FROM pos_user WHERE organization_id = ? AND id = ?",
    persist: "UPDATE local_credential SET algorithm = ?,secret_hash = ?,params_json = ?,policy_id = ?,state = ?,credential_version = ?,failed_login_attempts = ?,locked_until = strftime('%Y-%m-%dT%H:%M:%SZ',?,'unixepoch'),updated_at = strftime('%Y-%m-%dT%H:%M:%SZ',?,'unixepoch') WHERE organization_id = ? AND user_id = ? AND credential_kind = ? AND credential_version = ?",
    reset_state: "PASSWORD_RESET_REQUIRED",
};
