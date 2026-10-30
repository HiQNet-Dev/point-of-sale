//! Caracterización reproducible de Argon2id para D-AUTH-09.
//!
//! **No aprueba parámetros.** Mide candidatos sobre el hardware donde se ejecute
//! y emite JSON consumible por el expediente normativo.
//!
//! ```sh
//! cargo run -p pos-credentials --release --example argon2_bench -- \
//!   --rounds 5 --workers 1,2,4 --json target/argon2-bench.json
//! ```
//!
//! # Qué mide
//!
//! 1. La ruta pública real (`hash_password` + `verify_credential`) para las
//!    políticas registradas `LAB_V0`/`LAB_V1`.
//! 2. Candidatos adicionales construidos con `argon2::Params` **solo para medir**:
//!    el benchmark no puede eludir los límites de [`pos_credentials::Argon2Policy`]
//!    en código productivo, pero sí necesita cuantificar por qué un candidato
//!    queda fuera de política.
//! 3. Verificaciones concurrentes, para saber si un equipo pequeño satura la CPU.
//!
//! # Entorno
//!
//! CPU y memoria se detectan sin dependencias externas cuando el sistema lo
//! permite (`PROCESSOR_IDENTIFIER`, `/proc/cpuinfo`, `/proc/meminfo`). El
//! operador puede declarar las etiquetas del equipo objetivo con
//! `POS_BENCH_CPU` y `POS_BENCH_MEMORY_KIB`; es la vía prevista para ejecutar el
//! mismo binario en el hardware de una PyME.
//!
//! El pico de RSS solo se puede leer en sistemas con `/proc`; en otros se informa
//! `null` y se conserva el coste de memoria configurado (`m`).

use std::{
    env, fs,
    io::Write,
    time::{Duration, Instant},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use pos_core::identity::{CredentialAlgorithm, CredentialKind};
use pos_credentials::{Argon2Policy, VerifyRequest, hash_password, verify_credential};
use serde_json::{Value, json};

const PASSWORD: &str = "bench-only-passphrase";
const DEFAULT_ROUNDS: u32 = 5;
const SALT_BYTES: usize = 16;
const OUTPUT_BYTES: usize = 32;

/// Candidato medido con parámetros crudos.
struct Candidate {
    label: &'static str,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    /// `true` si la política registrada lo aceptaría para verificación.
    accepted_by_policy: bool,
}

const CANDIDATES: [Candidate; 4] = [
    Candidate {
        label: "cap-32m-2t-1p",
        memory_kib: 32_768,
        iterations: 2,
        lanes: 1,
        accepted_by_policy: true,
    },
    Candidate {
        label: "cap-32m-3t-1p",
        memory_kib: 32_768,
        iterations: 3,
        lanes: 1,
        accepted_by_policy: true,
    },
    Candidate {
        label: "spring-default-46m-1t-1p",
        memory_kib: 47_104,
        iterations: 1,
        lanes: 1,
        accepted_by_policy: false,
    },
    Candidate {
        label: "strong-64m-3t-1p",
        memory_kib: 65_536,
        iterations: 3,
        lanes: 1,
        accepted_by_policy: false,
    },
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rounds = DEFAULT_ROUNDS;
    let mut workers = vec![1_u32, 2, 4];
    let mut output: Option<String> = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--rounds" => {
                if let Some(value) = arguments.next() {
                    rounds = value.parse().unwrap_or(DEFAULT_ROUNDS).max(1);
                }
            }
            "--workers" => {
                if let Some(value) = arguments.next() {
                    let parsed: Vec<u32> = value
                        .split(',')
                        .filter_map(|item| item.trim().parse::<u32>().ok())
                        .filter(|item| *item > 0)
                        .collect();
                    if !parsed.is_empty() {
                        workers = parsed;
                    }
                }
            }
            "--json" => output = arguments.next(),
            _ => {}
        }
    }

    let policies = measure_registered_policies(rounds)?;
    let candidates = measure_candidates(rounds)?;
    let concurrency = measure_concurrency(&workers, rounds)?;
    let report = json!({
        "status": "OBSERVED",
        "normative": "D-AUTH-09 remains blocked until target hardware evidence is recorded",
        "environment": environment(),
        "rounds": rounds,
        "policies": policies,
        "candidates": candidates,
        "concurrency": concurrency,
        "memory": memory_profile(),
    });

    let rendered = serde_json::to_string_pretty(&report)?;
    println!("{rendered}");
    if let Some(path) = output {
        let mut file = fs::File::create(path)?;
        file.write_all(rendered.as_bytes())?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

fn environment() -> Value {
    json!({
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "logical_cpus": std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get),
        "cpu": env::var("POS_BENCH_CPU").ok().or_else(detect_cpu),
        "memory_total_kib": env::var("POS_BENCH_MEMORY_KIB").ok().and_then(|value| value.parse::<u64>().ok()).or_else(detect_total_memory_kib),
        "rust_version": option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
        "hardware_class": env::var("POS_BENCH_HARDWARE_CLASS").unwrap_or_else(|_| "UNSPECIFIED".to_owned()),
    })
}

fn detect_cpu() -> Option<String> {
    if let Ok(identifier) = env::var("PROCESSOR_IDENTIFIER") {
        return Some(identifier);
    }
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| {
        line.strip_prefix("model name")
            .and_then(|rest| rest.split_once(':'))
            .map(|(_, value)| value.trim().to_owned())
    })
}

fn detect_total_memory_kib() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    meminfo.lines().find_map(|line| {
        line.strip_prefix("MemTotal:")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
    })
}

fn memory_profile() -> Value {
    json!({
        "peak_rss_kib": peak_rss_kib(),
        "note": "El coste de memoria de Argon2id está gobernado por el parámetro m; el pico de RSS agregado depende de las verificaciones concurrentes.",
    })
}

fn peak_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
    })
}

fn measure_registered_policies(rounds: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let mut measured = Vec::new();
    for policy in [Argon2Policy::LAB_V0, Argon2Policy::LAB_V1] {
        let hash = hash_password(PASSWORD, policy)?;
        let started = Instant::now();
        for _ in 0..rounds {
            hash_password(PASSWORD, policy)?;
        }
        let hash_ms = per_round(started.elapsed(), rounds);

        let started = Instant::now();
        for _ in 0..rounds {
            let outcome = verify_credential(&VerifyRequest {
                purpose: CredentialKind::Password,
                algorithm: CredentialAlgorithm::Argon2id,
                stored: &hash,
                password: PASSWORD,
            })?;
            if !matches!(
                outcome,
                pos_credentials::VerificationOutcome::Verified { .. }
            ) {
                return Err("verificación de la ruta pública rechazada".into());
            }
        }
        let verify_ms = per_round(started.elapsed(), rounds);

        measured.push(json!({
            "id": policy.id(),
            "m_kib": policy.memory_kib(),
            "t": policy.iterations(),
            "p": policy.lanes(),
            "hash_ms": hash_ms,
            "verify_ms": verify_ms,
            "policy_cap_kib": pos_credentials::MAX_ARGON2_MEMORY_KIB,
        }));
    }
    Ok(Value::Array(measured))
}

fn measure_candidates(rounds: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let mut measured = Vec::new();
    for candidate in &CANDIDATES {
        let params = Params::new(
            candidate.memory_kib,
            candidate.iterations,
            candidate.lanes,
            Some(OUTPUT_BYTES),
        );
        let Ok(params) = params else {
            measured.push(json!({"label": candidate.label, "status": "INVALID_PARAMS"}));
            continue;
        };
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let salt = SaltString::generate(&mut OsRng);
        let Ok(encoded) = argon
            .hash_password(PASSWORD.as_bytes(), &salt)
            .map(|hash| hash.to_string())
        else {
            measured.push(json!({"label": candidate.label, "status": "HASH_FAILED"}));
            continue;
        };
        let Ok(parsed) = PasswordHash::new(&encoded) else {
            measured.push(json!({"label": candidate.label, "status": "PARSE_FAILED"}));
            continue;
        };

        let started = Instant::now();
        for _ in 0..rounds {
            let fresh = SaltString::generate(&mut OsRng);
            if argon.hash_password(PASSWORD.as_bytes(), &fresh).is_err() {
                return Err("hash del candidato falló".into());
            }
        }
        let hash_ms = per_round(started.elapsed(), rounds);

        let started = Instant::now();
        for _ in 0..rounds {
            if argon.verify_password(PASSWORD.as_bytes(), &parsed).is_err() {
                return Err("verificación del candidato falló".into());
            }
        }
        let verify_ms = per_round(started.elapsed(), rounds);

        measured.push(json!({
            "label": candidate.label,
            "m_kib": candidate.memory_kib,
            "t": candidate.iterations,
            "p": candidate.lanes,
            "hash_ms": hash_ms,
            "verify_ms": verify_ms,
            "accepted_by_current_policy": candidate.accepted_by_policy,
        }));
    }
    Ok(Value::Array(measured))
}

fn measure_concurrency(workers: &[u32], rounds: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let policy = Argon2Policy::LAB_V1;
    let stored = hash_password(PASSWORD, policy)?;
    let mut results = Vec::new();
    for &count in workers {
        let mut failures = 0_u32;
        let started = Instant::now();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..count)
                .map(|_| {
                    let reference = stored.clone();
                    scope.spawn(move || {
                        let mut local_failures = 0_u32;
                        for _ in 0..rounds {
                            match verify_credential(&VerifyRequest {
                                purpose: CredentialKind::Password,
                                algorithm: CredentialAlgorithm::Argon2id,
                                stored: &reference,
                                password: PASSWORD,
                            }) {
                                Ok(pos_credentials::VerificationOutcome::Verified { .. }) => {}
                                _ => local_failures += 1,
                            }
                        }
                        local_failures
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok(local) => failures += local,
                    Err(_) => failures += rounds,
                }
            }
        });
        let elapsed = started.elapsed();
        let operations = u64::from(count) * u64::from(rounds);
        results.push(json!({
            "policy": policy.id(),
            "workers": count,
            "operations": operations,
            "total_ms": millis(elapsed),
            "per_operation_ms": per_round(elapsed, u32::try_from(operations).unwrap_or(u32::MAX)),
            "wall_throughput_per_s": throughput(operations, elapsed),
            "failures": failures,
        }));
    }
    Ok(Value::Array(results))
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn per_round(duration: Duration, rounds: u32) -> f64 {
    millis(duration) / f64::from(rounds.max(1))
}

fn throughput(operations: u64, duration: Duration) -> f64 {
    if duration.is_zero() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        operations as f64 / duration.as_secs_f64()
    }
}

/// Mantiene el importe de SALT_BYTES documentado junto al contrato PHC.
const _: usize = SALT_BYTES;
