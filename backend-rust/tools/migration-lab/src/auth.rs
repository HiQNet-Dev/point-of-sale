//! Máquina de estados sintética de autenticación sin red; solo simula cambios en memoria.
//! No implementa autenticación productiva o de dominio, transacciones de BD ni tokens.

use std::time::Instant;

use argon2::{
    Algorithm, Argon2, Block, Params, Version,
    password_hash::{
        Error as HashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::LabError;

type Result<T> = std::result::Result<T, LabError>;

#[derive(Clone, Copy)]
struct Policy {
    id: &'static str,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
}

// Políticas explícitas del laboratorio: no son valores aprobados para producto ni Params::default().
const CURRENT: Policy = Policy {
    id: "lab-argon2-v1",
    memory_kib: 19_456,
    iterations: 2,
    lanes: 1,
};
const OLD: Policy = Policy {
    id: "lab-argon2-v0",
    memory_kib: 8_192,
    iterations: 1,
    lanes: 1,
};
const LEGACY_POLICY: &str = "legacy-import-v1";
const MAX_PHC_BYTES: usize = 256;
const MAX_MEMORY_KIB: u32 = 32_768;
const MAX_ITERATIONS: u32 = 3;
const MAX_LANES: u32 = 2;
const MAX_BCRYPT_COST: u32 = 12;
const NOW: u64 = 1_000;
const LOCK_THRESHOLD: u32 = 5;
const LOCK_SECONDS: u64 = 60;

fn error(code: &'static str) -> LabError {
    // No propagar errores criptográficos o de parseo: algunos de bcrypt incluyen el hash recibido.
    LabError::Invalid(format!("auth-lab: {code}"))
}

fn engine(policy: Policy) -> Result<Argon2<'static>> {
    let params = Params::new(policy.memory_kib, policy.iterations, policy.lanes, Some(32))
        .map_err(|_| error("POLICY_CONFIGURATION"))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn fresh_salt() -> Result<([u8; 16], SaltString)> {
    let mut bytes = [0_u8; 16];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| error("RNG_UNAVAILABLE"))?;
    let salt = SaltString::encode_b64(&bytes).map_err(|_| error("SALT_ENCODING"))?;
    Ok((bytes, salt))
}

fn encode(password: &str, policy: Policy) -> Result<String> {
    let (_, salt) = fresh_salt()?;
    engine(policy)?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| error("HASH_UNAVAILABLE"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialKind {
    LegacyBcrypt,
    Argon2id,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialState {
    Ready,
    ResetRequired,
}

// Sin Debug/Serialize para evitar que los hashes se filtren accidentalmente a los informes.
#[derive(Clone, PartialEq, Eq)]
struct State {
    active: bool,
    kind: CredentialKind,
    credential_state: CredentialState,
    encoded: String,
    policy_id: String,
    credential_version: u64,
    state_version: u64,
    failures: u32,
    locked_until: Option<u64>,
    local_sessions: u32,
}

impl State {
    fn imported(encoded: String, kind: CredentialKind, policy_id: &str) -> Self {
        Self {
            active: true,
            kind,
            credential_state: CredentialState::Ready,
            encoded,
            policy_id: policy_id.into(),
            credential_version: 7,
            state_version: 11,
            failures: 2,
            locked_until: None,
            local_sessions: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Authenticated,
    Invalid,
    Locked,
    Inactive,
    ResetRequired,
    InputRejected,
}

#[derive(Clone, Copy)]
struct Observation {
    outcome: Outcome,
    verifications: u32,
    rehashes: u32,
}

struct Transition {
    expected_version: u64,
    next: State,
    observation: Observation,
}

fn bounded_bcrypt(encoded: &str, policy_id: &str) -> bool {
    let bytes = encoded.as_bytes();
    if policy_id != LEGACY_POLICY
        || bytes.len() != 60
        || ![b"$2a$", b"$2b$", b"$2y$"]
            .iter()
            .any(|prefix| bytes.starts_with(*prefix))
        || !bytes[4..6].iter().all(u8::is_ascii_digit)
        || bytes[6] != b'$'
        || !bytes[7..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'.' || *b == b'/')
    {
        return false;
    }
    let cost = u32::from(bytes[4] - b'0') * 10 + u32::from(bytes[5] - b'0');
    // Limitar el coste ANTES de verificar: bcrypt admite costes de hasta 31.
    (4..=MAX_BCRYPT_COST).contains(&cost)
}

fn bounded_argon<'a>(encoded: &'a str, policy_id: &str) -> Option<(PasswordHash<'a>, Policy)> {
    if encoded.len() > MAX_PHC_BYTES {
        return None;
    }
    let policy = match policy_id {
        id if id == CURRENT.id => CURRENT,
        id if id == OLD.id => OLD,
        _ => return None, // Sin alternativa permisiva para políticas desconocidas, incluso con PHC válido.
    };
    let hash = PasswordHash::new(encoded).ok()?;
    let m = hash.params.get_decimal("m")?;
    let t = hash.params.get_decimal("t")?;
    let p = hash.params.get_decimal("p")?;
    if hash.algorithm.as_str() != "argon2id"
        || hash.version != Some(19)
        || hash.params.iter().count() != 3
        || !(8..=MAX_MEMORY_KIB).contains(&m)
        || !(1..=MAX_ITERATIONS).contains(&t)
        || !(1..=MAX_LANES).contains(&p)
        || (m, t, p) != (policy.memory_kib, policy.iterations, policy.lanes)
        || hash.hash.as_ref()?.len() != 32
    {
        return None;
    }
    let mut salt = [0_u8; 64];
    if hash.salt?.decode_b64(&mut salt).ok()?.len() != 16 {
        return None;
    }
    // Solo tras estos controles puede PasswordVerifier usar los parámetros autoritativos del PHC.
    Some((hash, policy))
}

fn prepare(snapshot: &State, password: &str, now: u64, target: Policy) -> Result<Transition> {
    if target.id != CURRENT.id
        || (target.memory_kib, target.iterations, target.lanes)
            != (CURRENT.memory_kib, CURRENT.iterations, CURRENT.lanes)
    {
        return Err(error("UNKNOWN_TARGET_POLICY"));
    }
    let mut next = snapshot.clone();
    let mut observation = Observation {
        outcome: Outcome::Invalid,
        verifications: 0,
        rehashes: 0,
    };
    observation.outcome = if !snapshot.active {
        Outcome::Inactive
    } else if snapshot.locked_until.is_some_and(|until| until > now) {
        Outcome::Locked
    } else if snapshot.credential_state == CredentialState::ResetRequired {
        Outcome::ResetRequired
    } else {
        authenticate(&mut next, password, now, target, &mut observation)?
    };
    if next != *snapshot {
        next.state_version = snapshot
            .state_version
            .checked_add(1)
            .ok_or_else(|| error("STATE_VERSION_EXHAUSTED"))?;
    }
    Ok(Transition {
        expected_version: snapshot.state_version,
        next,
        observation,
    })
}

fn require_reset(state: &mut State) -> Result<Outcome> {
    state.credential_state = CredentialState::ResetRequired;
    state.credential_version = state
        .credential_version
        .checked_add(1)
        .ok_or_else(|| error("CREDENTIAL_VERSION_EXHAUSTED"))?;
    Ok(Outcome::ResetRequired)
}

fn authenticate(
    state: &mut State,
    password: &str,
    now: u64,
    target: Policy,
    observation: &mut Observation,
) -> Result<Outcome> {
    let argon = match state.kind {
        CredentialKind::LegacyBcrypt if bounded_bcrypt(&state.encoded, &state.policy_id) => None,
        CredentialKind::Argon2id => match bounded_argon(&state.encoded, &state.policy_id) {
            Some(value) => Some(value),
            None => return require_reset(state),
        },
        _ => return require_reset(state),
    };
    // El límite se mide en bytes, no caracteres: se aceptan 72; nunca se truncan entradas mayores.
    // non_truncating_verify de bcrypt 0.17.1 también rechaza 72 al contar el NUL que añade;
    // por eso se usa verify únicamente después del control explícito de <=72 bytes.
    if password.len() > 1024
        || password.as_bytes().contains(&0)
        || (state.kind == CredentialKind::LegacyBcrypt && password.len() > 72)
    {
        return Ok(Outcome::InputRejected);
    }
    observation.verifications = 1;
    let valid = if let Some((hash, policy)) = argon {
        match engine(policy)?.verify_password(password.as_bytes(), &hash) {
            Ok(()) => true,
            Err(HashError::Password) => false,
            Err(_) => return require_reset(state),
        }
    } else {
        match bcrypt::verify(password.as_bytes(), &state.encoded) {
            Ok(valid) => valid,
            Err(_) => return require_reset(state),
        }
    };
    if !valid {
        state.failures = state
            .failures
            .checked_add(1)
            .ok_or_else(|| error("FAILURE_COUNTER_EXHAUSTED"))?;
        if state.failures >= LOCK_THRESHOLD {
            state.locked_until = Some(
                now.checked_add(LOCK_SECONDS)
                    .ok_or_else(|| error("CLOCK_EXHAUSTED"))?,
            );
        }
        return Ok(Outcome::Invalid);
    }
    if state.kind == CredentialKind::LegacyBcrypt || state.policy_id != target.id {
        // Preparar todo sobre la copia privada; el commit en memoria no aplica cambios parciales.
        // Esta simulación no demuestra atomicidad de una transacción de BD.
        state.encoded = encode(password, target)?;
        state.kind = CredentialKind::Argon2id;
        state.policy_id = target.id.into();
        state.credential_version = state
            .credential_version
            .checked_add(1)
            .ok_or_else(|| error("CREDENTIAL_VERSION_EXHAUSTED"))?;
        observation.rehashes = 1;
    }
    state.failures = 0;
    state.locked_until = None;
    state.local_sessions = state
        .local_sessions
        .checked_add(1)
        .ok_or_else(|| error("SESSION_COUNTER_EXHAUSTED"))?;
    Ok(Outcome::Authenticated)
}

fn commit(state: &mut State, transition: Transition) -> Result<Observation> {
    if state.state_version != transition.expected_version {
        return Err(error("STATE_VERSION_CONFLICT"));
    }
    *state = transition.next;
    Ok(transition.observation)
}

fn login(state: &mut State, password: &str) -> Result<Observation> {
    let transition = prepare(state, password, NOW, CURRENT)?;
    commit(state, transition)
}

// No se parsean ni verifican tokens Java; el estado local no contiene claves JWT ni tokens.
fn accepts_java_session(_untrusted_handle: &[u8]) -> bool {
    false
}

#[derive(Deserialize)]
struct Fixtures {
    synthetic_only: bool,
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
struct Vector {
    label: String,
    variant: String,
    password: String,
    utf8_bytes: usize,
    encoded: String,
}

fn fixtures() -> Result<Fixtures> {
    let fixture: Fixtures =
        serde_json::from_str(include_str!("../fixtures/auth/spring-vectors.json"))
            .map_err(|_| error("SYNTHETIC_FIXTURE_INVALID"))?;
    if !fixture.synthetic_only || fixture.vectors.len() != 15 {
        return Err(error("SYNTHETIC_FIXTURE_INVALID"));
    }
    Ok(fixture)
}

fn legacy() -> Result<(State, String)> {
    let vector = fixtures()?
        .vectors
        .into_iter()
        .next()
        .ok_or_else(|| error("SYNTHETIC_FIXTURE_EMPTY"))?;
    Ok((
        State::imported(vector.encoded, CredentialKind::LegacyBcrypt, LEGACY_POLICY),
        vector.password,
    ))
}

fn argon(policy: Policy) -> Result<(State, String)> {
    let password = "synthetic-argon-only-é".to_string();
    let encoded = encode(&password, policy)?;
    Ok((
        State::imported(encoded, CredentialKind::Argon2id, policy.id),
        password,
    ))
}

fn case_01() -> Result<bool> {
    for vector in fixtures()?.vectors {
        if vector.password.len() != vector.utf8_bytes
            || !["ordinary", "ascii71", "ascii72", "utf8_71", "utf8_72"]
                .contains(&vector.label.as_str())
            || !["2a", "2b", "2y"].contains(&vector.variant.as_str())
            || !vector.encoded.starts_with(&format!("${}$", vector.variant))
        {
            return Ok(false);
        }
        let mut state =
            State::imported(vector.encoded, CredentialKind::LegacyBcrypt, LEGACY_POLICY);
        let before = state.clone();
        let result = login(&mut state, &vector.password)?;
        let migrated_valid =
            bounded_argon(&state.encoded, &state.policy_id).is_some_and(|(hash, _)| {
                engine(CURRENT)
                    .is_ok_and(|e| e.verify_password(vector.password.as_bytes(), &hash).is_ok())
            });
        if result.outcome != Outcome::Authenticated
            || result.rehashes != 1
            || state.kind != CredentialKind::Argon2id
            || !migrated_valid
            || state.encoded == before.encoded
            || state.credential_version != before.credential_version + 1
            || state.state_version != before.state_version + 1
            || state.failures != 0
            || state.locked_until.is_some()
            || state.local_sessions != 1
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn case_02() -> Result<bool> {
    let (mut state, _) = legacy()?;
    let before = state.clone();
    let result = login(&mut state, "wrong-synthetic-input")?;
    Ok(result.outcome == Outcome::Invalid
        && result.rehashes == 0
        && result.verifications == 1
        && state.encoded == before.encoded
        && state.kind == before.kind
        && state.policy_id == before.policy_id
        && state.credential_version == before.credential_version
        && state.state_version == before.state_version + 1
        && state.failures == before.failures + 1
        && state.local_sessions == 0
        && state.locked_until.is_none())
}

fn case_03() -> Result<bool> {
    let (mut state, password) = argon(CURRENT)?;
    let before = state.clone();
    let result = login(&mut state, &password)?;
    Ok(result.outcome == Outcome::Authenticated
        && result.rehashes == 0
        && result.verifications == 1
        && state.encoded == before.encoded
        && state.credential_version == before.credential_version
        && state.policy_id == CURRENT.id
        && state.failures == 0
        && state.local_sessions == 1
        && state.state_version == before.state_version + 1)
}

fn case_04() -> Result<bool> {
    let (mut state, password) = argon(OLD)?;
    let before = state.clone();
    let result = login(&mut state, &password)?;
    let verifies = bounded_argon(&state.encoded, &state.policy_id).is_some_and(|(hash, _)| {
        engine(CURRENT).is_ok_and(|e| e.verify_password(password.as_bytes(), &hash).is_ok())
    });
    Ok(result.outcome == Outcome::Authenticated
        && result.rehashes == 1
        && verifies
        && state.encoded != before.encoded
        && state.policy_id == CURRENT.id
        && state.credential_version == before.credential_version + 1
        && state.state_version == before.state_version + 1
        && state.failures == 0
        && state.local_sessions == 1)
}

fn case_05() -> Result<bool> {
    let (mut state, password) = legacy()?;
    state.locked_until = Some(NOW + 60);
    let before = state.clone();
    let result = login(&mut state, &password)?;
    Ok(result.outcome == Outcome::Locked
        && result.verifications == 0
        && result.rehashes == 0
        && state == before)
}

fn case_06() -> Result<bool> {
    for (mut state, password) in [legacy()?, argon(OLD)?] {
        state.active = false;
        let before = state.clone();
        let result = login(&mut state, &password)?;
        if result.outcome != Outcome::Inactive
            || result.verifications != 0
            || result.rehashes != 0
            || state != before
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn case_07() -> Result<bool> {
    let (base, password) = argon(CURRENT)?;
    for (encoded, kind, policy) in [
        (
            "not-a-hash".to_string(),
            CredentialKind::LegacyBcrypt,
            LEGACY_POLICY,
        ),
        (base.encoded.clone(), CredentialKind::Unknown, CURRENT.id),
        (
            base.encoded.clone(),
            CredentialKind::Argon2id,
            "unknown-policy",
        ),
        (
            "$argon2id$broken".into(),
            CredentialKind::Argon2id,
            CURRENT.id,
        ),
    ] {
        let mut state = State::imported(encoded, kind, policy);
        let before = state.clone();
        let result = login(&mut state, &password)?;
        if result.outcome != Outcome::ResetRequired
            || result.rehashes != 0
            || state.credential_state != CredentialState::ResetRequired
            || state.encoded != before.encoded
            || state.failures != before.failures
            || state.local_sessions != 0
            || state.credential_version != before.credential_version + 1
            || state.state_version != before.state_version + 1
        {
            return Ok(false);
        }
        let after = state.clone();
        let retry = login(&mut state, &password)?;
        if retry.outcome != Outcome::ResetRequired || retry.verifications != 0 || state != after {
            return Ok(false);
        }
    }
    Ok(true)
}

fn case_08() -> Result<bool> {
    let (mut state, password) = legacy()?;
    let before = state.clone();
    // Identificador opaco sintético, NO un JWT firmado por Java ni un refresh token real.
    if accepts_java_session(b"synthetic-java-handle-not-a-jwt")
        || accepts_java_session(b"")
        || state != before
        || state.local_sessions != 0
    {
        return Ok(false);
    }
    let result = login(&mut state, &password)?;
    Ok(result.outcome == Outcome::Authenticated && state.local_sessions == 1)
}

fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() * percent).div_ceil(100).saturating_sub(1)]
}

fn benchmark(policy: Policy) -> Result<Value> {
    const SAMPLES: usize = 20;
    let engine = engine(policy)?;
    let password = "synthetic-benchmark-only";
    let reference = encode(password, policy)?;
    let (parsed, _) = bounded_argon(&reference, policy.id).ok_or_else(|| error("BENCHMARK_PHC"))?;
    // Un calentamiento sin medir por operación; sin umbral de aprobación de UX/seguridad.
    // Medir en este host no aprueba la política para producto.
    engine
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| error("BENCHMARK_VERIFY"))?;
    let _ = encode(password, policy)?;
    let mut hashing_ms = Vec::with_capacity(SAMPLES);
    let mut verifying_ms = Vec::with_capacity(SAMPLES);
    let mut workspace_bytes = 0;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let (salt, _) = fresh_salt()?;
        // El búfer explícito de la biblioteca permite medir la asignación real de bloques Argon2,
        // sin atribuir incorrectamente a una política el pico RSS de todo el proceso.
        let mut blocks = vec![Block::default(); engine.params().block_count()];
        workspace_bytes = std::mem::size_of_val(blocks.as_slice());
        let mut output = [0_u8; 32];
        engine
            .hash_password_into_with_memory(password.as_bytes(), &salt, &mut output, &mut blocks)
            .map_err(|_| error("BENCHMARK_HASH"))?;
        hashing_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        drop(blocks);
        let start = Instant::now();
        engine
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| error("BENCHMARK_VERIFY"))?;
        verifying_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    hashing_ms.sort_by(f64::total_cmp);
    verifying_ms.sort_by(f64::total_cmp);
    Ok(json!({
        "policy_id": policy.id,
        "m_kib": policy.memory_kib, "t": policy.iterations, "p": policy.lanes,
        "samples_per_operation": SAMPLES, "warmup_per_operation": 1,
        "hash_ms": {"p50": percentile(&hashing_ms, 50), "p95": percentile(&hashing_ms, 95)},
        "verify_ms": {"p50": percentile(&verifying_ms, 50), "p95": percentile(&verifying_ms, 95)},
        "memory": {"workspace_bytes_measured": workspace_bytes,
            "method": "size_of_val of actual RustCrypto Block allocation per hash sample",
            "includes_process_overhead": false, "peak_rss_bytes": null},
    }))
}

fn run_sync() -> Result<Value> {
    let checks: [fn() -> Result<bool>; 8] = [
        case_01, case_02, case_03, case_04, case_05, case_06, case_07, case_08,
    ];
    let cases: Vec<_> = checks.into_iter().enumerate().map(|(index, check)| {
        // Solo campos de salida permitidos y textos fijos; sin serializar estado ni excepciones.
        let passed = matches!(check(), Ok(true));
        json!({"id": format!("AUTH-MIG-{:02}", index + 1),
            "status": if passed { "PASSED" } else { "FAILED" }, "scope": "synthetic_technical",
            "detail": if passed { "SYNTHETIC_INVARIANTS_VERIFIED" } else { "SYNTHETIC_CHECK_FAILED" }})
    }).collect();
    let passed = cases.iter().all(|case| case["status"] == "PASSED");
    Ok(json!({
        "status": if passed { "PASSED" } else { "FAILED" },
        "scope": "synthetic_technical", "normative_status": "BLOCKED", "as09_normative": false,
        "limitation": "Offline in-memory fixture only; full import, productive auth/session storage and AS-09 not implemented",
        "policy": {"policy_id": CURRENT.id, "selection": "EXPLICIT_LAB_ONLY", "commercial_approved": false,
            "algorithm": "argon2id", "version": 19, "m_kib": CURRENT.memory_kib,
            "t": CURRENT.iterations, "p": CURRENT.lanes, "output_bytes": 32, "salt_bytes": 16,
            "basis": "F4-19 OWASP reference floor; benchmark observation is not product approval"},
        "verifier_limits": {"phc_bytes": MAX_PHC_BYTES, "m_kib_max": MAX_MEMORY_KIB,
            "t_max": MAX_ITERATIONS, "p_max": MAX_LANES, "bcrypt_cost_max": MAX_BCRYPT_COST,
            "registered_policy_and_exact_params_required": true, "bcrypt_password_bytes_max": 72},
        "cases": cases,
        "benchmark": {"host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                "logical_cpus_available": std::thread::available_parallelism().ok().map(|n| n.get()),
                "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
                "product_hardware": false, "scope": "CURRENT_EXECUTION_HOST_ONLY"},
            "method": "sequential single-worker wall-clock, nearest-rank percentiles; hash includes RNG/workspace allocation, verify uses guarded PHC",
            "policies": [benchmark(OLD)?, benchmark(CURRENT)?]},
    }))
}

/// Ejecuta los controles sintéticos de CPU fuera del ejecutor asíncrono.
/// Devuelve solo evidencia técnica permitida; no acredita atomicidad de BD ni aprobación de producto.
pub(crate) async fn run() -> Result<Value> {
    tokio::task::spawn_blocking(run_sync)
        .await
        .map_err(|_| error("WORKER_FAILED"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn auth_mig_report_has_exactly_eight_passing_technical_cases() -> Result<()> {
        let report = run().await?;
        let cases = report["cases"]
            .as_array()
            .ok_or_else(|| error("MISSING_CASES"))?;
        assert_eq!(cases.len(), 8);
        for (index, case) in cases.iter().enumerate() {
            assert_eq!(case["id"], format!("AUTH-MIG-{:02}", index + 1));
            assert_eq!(case["status"], "PASSED");
            assert_eq!(case["scope"], "synthetic_technical");
        }
        assert_eq!(report["normative_status"], "BLOCKED");
        assert_eq!(report["policy"]["commercial_approved"], false);
        assert_eq!(report["benchmark"]["host"]["product_hardware"], false);
        for (index, policy) in [OLD, CURRENT].iter().enumerate() {
            let measured = &report["benchmark"]["policies"][index];
            assert_eq!(
                measured["memory"]["workspace_bytes_measured"],
                u64::from(policy.memory_kib) * 1024
            );
            for operation in ["hash_ms", "verify_ms"] {
                let p50 = measured[operation]["p50"]
                    .as_f64()
                    .ok_or_else(|| error("MISSING_P50"))?;
                let p95 = measured[operation]["p95"]
                    .as_f64()
                    .ok_or_else(|| error("MISSING_P95"))?;
                assert!(p50 > 0.0 && p95 >= p50);
            }
        }
        let text = serde_json::to_string(&report).map_err(|_| error("REPORT_ENCODING"))?;
        for vector in fixtures()?.vectors {
            assert!(!text.contains(&vector.password));
            assert!(!text.contains(&vector.encoded));
            assert!(!text.contains(&vector.encoded[7..29]));
        }
        for forbidden in [
            "$argon2",
            "$2a$",
            "$2b$",
            "$2y$",
            "synthetic-java-handle-not-a-jwt",
            "synthetic-argon-only-é",
            "synthetic-benchmark-only",
        ] {
            assert!(!text.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn stale_upgrade_cannot_overwrite_a_committed_login_or_create_another_session() -> Result<()> {
        let (mut state, password) = legacy()?;
        let first = prepare(&state, &password, NOW, CURRENT)?;
        let stale = prepare(&state, &password, NOW, CURRENT)?;
        let untouched = state.clone();
        assert!(state == untouched && state.local_sessions == 0);
        commit(&mut state, first)?;
        let committed = state.clone();
        assert!(commit(&mut state, stale).is_err());
        assert!(state == committed && state.local_sessions == 1);
        Ok(())
    }

    #[test]
    fn utf8_and_ascii_above_72_bytes_are_rejected_without_mutation() -> Result<()> {
        for vector in fixtures()?
            .vectors
            .into_iter()
            .filter(|v| v.utf8_bytes == 72)
        {
            let mut state =
                State::imported(vector.encoded, CredentialKind::LegacyBcrypt, LEGACY_POLICY);
            let before = state.clone();
            for input in [
                format!("{}x", vector.password),
                format!("{}é", vector.password),
            ] {
                let result = login(&mut state, &input)?;
                assert_eq!(result.outcome, Outcome::InputRejected);
                assert!(result.verifications == 0 && result.rehashes == 0 && state == before);
            }
        }
        // Truncar a 72 bytes una entrada de 71 bytes ASCII seguida de un carácter multibyte rompería UTF-8.
        let (mut state, _) = legacy()?;
        let before = state.clone();
        assert_eq!(
            login(&mut state, &format!("{}é", "a".repeat(71)))?.outcome,
            Outcome::InputRejected
        );
        assert!(state == before);
        Ok(())
    }

    #[test]
    fn resource_and_policy_guards_reject_before_expensive_verification() -> Result<()> {
        let (base, password) = argon(CURRENT)?;
        let mut adversarial = vec![
            (base.encoded.replace("m=19456", "m=4294967295"), CURRENT.id),
            (base.encoded.replace("t=2", "t=4294967295"), CURRENT.id),
            (base.encoded.replace("p=1", "p=4294967295"), CURRENT.id),
            (base.encoded.replace("v=19", "v=16"), CURRENT.id),
            (base.encoded.replace("argon2id", "argon2i"), CURRENT.id),
            (base.encoded.replace("m=19456", "m=8192"), CURRENT.id),
            (base.encoded.replace("p=1", "p=1,keyid=YWJj"), CURRENT.id),
            ("x".repeat(MAX_PHC_BYTES + 1), CURRENT.id),
            (base.encoded.clone(), "unknown-policy"),
            (base.encoded.clone(), OLD.id),
        ];
        // También se limitan los tamaños de salt y salida, no solo m/t/p.
        let parts: Vec<_> = base.encoded.split('$').collect();
        adversarial.push((
            format!(
                "{}${}${}${}${}${}",
                parts[0], parts[1], parts[2], parts[3], "YQ", parts[5]
            ),
            CURRENT.id,
        ));
        adversarial.push((
            format!(
                "{}${}${}${}${}${}",
                parts[0], parts[1], parts[2], parts[3], parts[4], "YQ"
            ),
            CURRENT.id,
        ));
        for (encoded, policy) in adversarial {
            let mut state = State::imported(encoded, CredentialKind::Argon2id, policy);
            let result = login(&mut state, &password)?;
            assert_eq!(result.outcome, Outcome::ResetRequired);
            assert!(result.verifications == 0 && result.rehashes == 0 && state.local_sessions == 0);
        }
        let (base, _) = legacy()?;
        for encoded in [
            base.encoded.replacen("$04$", "$31$", 1),
            base.encoded.replacen("$2a$", "$2x$", 1),
        ] {
            let mut state = State::imported(encoded, CredentialKind::LegacyBcrypt, LEGACY_POLICY);
            let result = login(&mut state, "irrelevant")?;
            assert_eq!(result.outcome, Outcome::ResetRequired);
            assert_eq!(result.verifications, 0);
        }
        Ok(())
    }

    #[test]
    fn failures_reach_lock_threshold_and_future_lock_preserves_every_field() -> Result<()> {
        let (mut state, password) = legacy()?;
        state.failures = LOCK_THRESHOLD - 1;
        assert_eq!(login(&mut state, "wrong")?.outcome, Outcome::Invalid);
        assert_eq!(state.failures, LOCK_THRESHOLD);
        assert_eq!(state.locked_until, Some(NOW + LOCK_SECONDS));
        let before = state.clone();
        assert_eq!(login(&mut state, &password)?.outcome, Outcome::Locked);
        assert!(state == before);
        let transition = prepare(&state, &password, NOW + LOCK_SECONDS, CURRENT)?;
        assert_eq!(
            commit(&mut state, transition)?.outcome,
            Outcome::Authenticated
        );
        assert!(state.failures == 0 && state.locked_until.is_none());
        Ok(())
    }

    #[test]
    fn invalid_argon_password_never_rehashes_and_preserves_versions_of_hash() -> Result<()> {
        for policy in [OLD, CURRENT] {
            let (mut state, _) = argon(policy)?;
            let before = state.clone();
            let result = login(&mut state, "wrong")?;
            assert_eq!(result.outcome, Outcome::Invalid);
            assert!(result.rehashes == 0 && state.encoded == before.encoded);
            assert!(
                state.policy_id == before.policy_id
                    && state.credential_version == before.credential_version
            );
            assert!(state.failures == before.failures + 1 && state.local_sessions == 0);
        }
        Ok(())
    }

    #[test]
    fn overflow_or_unknown_target_cannot_partially_commit_upgrade() -> Result<()> {
        let (mut state, password) = legacy()?;
        state.credential_version = u64::MAX;
        let before = state.clone();
        assert!(login(&mut state, &password).is_err());
        assert!(state == before);
        assert!(prepare(&state, &password, NOW, OLD).is_err());
        assert!(state == before);
        state.credential_version = 7;
        state.state_version = u64::MAX;
        let before = state.clone();
        assert!(login(&mut state, "wrong").is_err());
        assert!(state == before);
        Ok(())
    }

    #[test]
    fn stale_upgrade_cannot_resurrect_a_concurrently_disabled_account() -> Result<()> {
        let (mut state, password) = legacy()?;
        let stale = prepare(&state, &password, NOW, CURRENT)?;
        // Simular una actualización versionada competidora, no una desactivación productiva.
        state.active = false;
        state.state_version += 1;
        let disabled = state.clone();
        assert!(commit(&mut state, stale).is_err());
        assert!(state == disabled && state.local_sessions == 0);
        Ok(())
    }

    #[test]
    fn nul_and_oversized_argon_inputs_do_not_mutate_or_invoke_verification() -> Result<()> {
        for (mut state, _) in [legacy()?, argon(CURRENT)?] {
            let before = state.clone();
            for password in ["synthetic\0input".to_string(), "a".repeat(1025)] {
                let result = login(&mut state, &password)?;
                assert_eq!(result.outcome, Outcome::InputRejected);
                assert!(result.verifications == 0 && result.rehashes == 0 && state == before);
            }
        }
        Ok(())
    }

    #[test]
    fn errors_expose_only_fixed_codes_not_crypto_input() -> Result<()> {
        let (state, password) = legacy()?;
        let result = prepare(&state, &password, NOW, OLD);
        match result {
            Err(error) => {
                let message = error.to_string();
                assert_eq!(message, "auth-lab: UNKNOWN_TARGET_POLICY");
                assert!(!message.contains(&state.encoded) && !message.contains(&password));
            }
            Ok(_) => return Err(error("EXPECTED_POLICY_REJECTION")),
        }
        Ok(())
    }

    #[test]
    fn fresh_hashes_use_distinct_csprng_salts_without_weakening_policy() -> Result<()> {
        let first = encode("synthetic-salt-check", CURRENT)?;
        let second = encode("synthetic-salt-check", CURRENT)?;
        let (first, _) = bounded_argon(&first, CURRENT.id).ok_or_else(|| error("FIRST_PHC"))?;
        let (second, _) = bounded_argon(&second, CURRENT.id).ok_or_else(|| error("SECOND_PHC"))?;
        assert!(first.salt != second.salt);
        Ok(())
    }
}
