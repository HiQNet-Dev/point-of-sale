//! Caracterización reproducible de Argon2id para D-AUTH-09.
//!
//! **No aprueba parámetros.** Mide candidatos sobre el hardware donde se ejecute y
//! emite JSON consumible por el expediente normativo.
//!
//! ```sh
//! cargo run -p pos-credentials --release --example argon2_bench -- \
//!   --rounds 20 --workers 1,4,16 --burst 32 --json target/argon2-bench.json
//! ```
//!
//! # Qué mide
//!
//! 1. La ruta pública real (`hash_password` + `verify_credential`) para las
//!    políticas registradas `LAB_V0`/`LAB_V1`, con latencias individuales
//!    (p50/p95/p99) de generación y verificación por separado.
//! 2. Candidatos construidos con `argon2::Params` **solo para medir**, incluidos
//!    los que la política actual rechaza.
//! 3. Carga concurrente y una ráfaga de intentos simultáneos, que modela una
//!    avalancha de logins; el benchmark informa saturación, no aplica límites.
//! 4. Pico de RSS cuando el sistema expone `/proc`.
//! 5. Aislamiento de memoria por proceso (`--memory-isolation`): ejecuta cada
//!    escenario en un proceso hijo nuevo para separar memoria base, coste de una
//!    operación Argon2id y memoria retenida por el asignador tras concurrencia.
//!
//! # Entorno
//!
//! CPU y memoria se detectan sin dependencias externas cuando el sistema lo
//! permite. El operador declara el equipo objetivo con `POS_BENCH_CPU`,
//! `POS_BENCH_MEMORY_KIB` y `POS_BENCH_HARDWARE_CLASS`. Los límites de CPU/RAM se
//! aplican **externamente** (contenedor, `systemd`, `taskset`) y deben
//! registrarse en la clase de hardware, no simularse dentro del binario.
//!
//! El límite de intentos simultáneos es una política de aplicación, no
//! criptográfica: este instrumento mide el coste real de atenderlos.

use std::{
    env, fs,
    io::Write,
    process::Command,
    time::{Duration, Instant},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use pos_core::identity::{CredentialAlgorithm, CredentialKind};
use pos_credentials::{
    Argon2Policy, VerificationOutcome, VerifyRequest, hash_password, verify_credential,
};
use serde_json::{Value, json};

const PASSWORD: &str = "bench-only-passphrase";
const DEFAULT_ROUNDS: u32 = 20;
const DEFAULT_BURST: u32 = 32;
const SALT_BYTES: usize = 16;
const OUTPUT_BYTES: usize = 32;

struct Candidate {
    label: &'static str,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
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
    let mut workers = vec![1_u32, 4, 16];
    let mut burst = DEFAULT_BURST;
    let mut output: Option<String> = None;
    let mut isolate_memory = false;
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
            "--burst" => {
                if let Some(value) = arguments.next() {
                    burst = value.parse().unwrap_or(DEFAULT_BURST).max(1);
                }
            }
            "--memory-isolation" => isolate_memory = true,
            "--memory-child" => {
                // Modo interno: un escenario por proceso, sin estado compartido.
                let workers = arguments
                    .next()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(0);
                return memory_child(workers);
            }
            "--json" => output = arguments.next(),
            _ => {}
        }
    }

    let policies = measure_registered_policies(rounds)?;
    let after_policies = peak_rss_kib();
    let candidates = measure_candidates(rounds)?;
    let after_candidates = peak_rss_kib();
    let concurrency = measure_concurrency(&workers, rounds)?;
    let after_concurrency = peak_rss_kib();
    let burst_report = measure_burst(burst)?;
    let memory_isolation = if isolate_memory {
        measure_memory_isolation()?
    } else {
        Value::Null
    };
    let report = json!({
        "status": "OBSERVED",
        "normative": "D-AUTH-09 remains blocked until target hardware evidence is recorded and the holder approves",
        "environment": environment(),
        "input": { "rounds": rounds, "workers": workers, "burst": burst },
        "policies": policies,
        "candidates": candidates,
        "concurrency": concurrency,
        "burst": burst_report,
        "memory": memory_profile(after_policies, after_candidates, after_concurrency),
        "memory_isolation": memory_isolation,
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
        "external_limits": env::var("POS_BENCH_LIMITS").unwrap_or_else(|_| "UNSPECIFIED".to_owned()),
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

/// Pico acumulado de RSS por fase; `VmHWM` es monótono, así que un valor que no
/// crece indica que la fase no aumentó el pico.
fn memory_profile(
    after_policies: Option<u64>,
    after_candidates: Option<u64>,
    after_concurrency: Option<u64>,
) -> Value {
    json!({
        "peak_rss_kib": peak_rss_kib(),
        "after_policies_kib": after_policies,
        "after_candidates_kib": after_candidates,
        "after_concurrency_kib": after_concurrency,
        "note": "VmHWM es el pico del proceso y es monótono; el coste de Argon2id está gobernado por m, pero el asignador puede retener memoria por hilo bajo concurrencia.",
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

/// Un escenario de memoria por proceso: base, una operación y concurrencia.
///
/// Se ejecuta en un hijo porque `VmHWM` es monótono y el asignador reutiliza
/// bloques: medir todo en el mismo proceso confunde coste criptográfico con
/// retención del asignador.
fn memory_child(workers: u32) -> Result<(), Box<dyn std::error::Error>> {
    let base = peak_rss_kib();
    let policy = Argon2Policy::LAB_V1;
    let started = Instant::now();
    let stored = hash_password(PASSWORD, policy)?;
    let hash_ms = millis(started.elapsed());
    let after_hash = peak_rss_kib();

    let mut failures = 0_u32;
    let verify_started = Instant::now();
    if workers > 0 {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    let reference = &stored;
                    scope.spawn(move || {
                        matches!(
                            verify_credential(&VerifyRequest {
                                purpose: CredentialKind::Password,
                                algorithm: CredentialAlgorithm::Argon2id,
                                stored: reference,
                                password: PASSWORD,
                            }),
                            Ok(VerificationOutcome::Verified { .. })
                        )
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok(true) => {}
                    _ => failures += 1,
                }
            }
        });
    }
    let verify_ms = millis(verify_started.elapsed());
    let peak = peak_rss_kib();
    println!(
        "{}",
        json!({
            "workers": workers,
            "policy": policy.id(),
            "m_kib": policy.memory_kib(),
            "base_rss_kib": base,
            "after_hash_rss_kib": after_hash,
            "peak_rss_kib": peak,
            "crypto_cost_kib": after_hash.zip(base).map(|(after, before)| after.saturating_sub(before)),
            "allocator_retained_kib": peak.zip(after_hash).map(|(after, hash)| after.saturating_sub(hash)),
            "hash_ms": hash_ms,
            "verify_wall_ms": verify_ms,
            "failures": failures,
            "rss_available": peak.is_some(),
        })
    );
    Ok(())
}

/// Ejecuta el escenario de memoria en procesos hijos independientes.
fn measure_memory_isolation() -> Result<Value, Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let mut scenarios = Vec::new();
    for workers in [0_u32, 1, 2, 4, 8] {
        let output = Command::new(&executable)
            .arg("--memory-child")
            .arg(workers.to_string())
            .output()?;
        if !output.status.success() {
            scenarios.push(json!({ "workers": workers, "status": "FAILED" }));
            continue;
        }
        let rendered = String::from_utf8(output.stdout)?;
        match serde_json::from_str::<Value>(rendered.trim()) {
            Ok(value) => scenarios.push(value),
            Err(_) => scenarios.push(json!({ "workers": workers, "status": "UNPARSEABLE" })),
        }
    }
    Ok(json!({
        "scenarios": scenarios,
        "note": "Cada escenario es un proceso nuevo: base, coste de una operación y retención del asignador quedan separados. `rss_available=false` indica que el sistema no expone /proc; la evidencia debe repetirse en el equipo objetivo.",
    }))
}

fn measure_registered_policies(rounds: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let mut measured = Vec::new();
    for policy in [Argon2Policy::LAB_V0, Argon2Policy::LAB_V1] {
        let mut hash_samples = Vec::with_capacity(rounds as usize);
        for _ in 0..rounds {
            let started = Instant::now();
            let hash = hash_password(PASSWORD, policy)?;
            hash_samples.push(millis(started.elapsed()));
            drop(hash);
        }
        let stored = hash_password(PASSWORD, policy)?;
        let mut verify_samples = Vec::with_capacity(rounds as usize);
        for _ in 0..rounds {
            let started = Instant::now();
            let outcome = verify_credential(&VerifyRequest {
                purpose: CredentialKind::Password,
                algorithm: CredentialAlgorithm::Argon2id,
                stored: &stored,
                password: PASSWORD,
            })?;
            verify_samples.push(millis(started.elapsed()));
            if !matches!(outcome, VerificationOutcome::Verified { .. }) {
                return Err("verificación de la ruta pública rechazada".into());
            }
        }
        measured.push(json!({
            "id": policy.id(),
            "m_kib": policy.memory_kib(),
            "t": policy.iterations(),
            "p": policy.lanes(),
            "hash": distribution(&hash_samples),
            "verify": distribution(&verify_samples),
            "policy_cap_kib": pos_credentials::MAX_ARGON2_MEMORY_KIB,
        }));
    }
    Ok(Value::Array(measured))
}

fn measure_candidates(rounds: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let mut measured = Vec::new();
    for candidate in &CANDIDATES {
        let Ok(params) = Params::new(
            candidate.memory_kib,
            candidate.iterations,
            candidate.lanes,
            Some(OUTPUT_BYTES),
        ) else {
            measured.push(json!({"label": candidate.label, "status": "INVALID_PARAMS"}));
            continue;
        };
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut hash_samples = Vec::with_capacity(rounds as usize);
        let mut verify_samples = Vec::with_capacity(rounds as usize);
        let mut encoded = String::new();
        let mut failed = false;
        for _ in 0..rounds {
            let salt = SaltString::generate(&mut OsRng);
            let started = Instant::now();
            match argon.hash_password(PASSWORD.as_bytes(), &salt) {
                Ok(hash) => {
                    hash_samples.push(millis(started.elapsed()));
                    encoded = hash.to_string();
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            measured.push(json!({"label": candidate.label, "status": "HASH_FAILED"}));
            continue;
        }
        let Ok(parsed) = PasswordHash::new(&encoded) else {
            measured.push(json!({"label": candidate.label, "status": "PARSE_FAILED"}));
            continue;
        };
        for _ in 0..rounds {
            let started = Instant::now();
            if argon.verify_password(PASSWORD.as_bytes(), &parsed).is_err() {
                failed = true;
                break;
            }
            verify_samples.push(millis(started.elapsed()));
        }
        if failed {
            measured.push(json!({"label": candidate.label, "status": "VERIFY_FAILED"}));
            continue;
        }
        measured.push(json!({
            "label": candidate.label,
            "m_kib": candidate.memory_kib,
            "t": candidate.iterations,
            "p": candidate.lanes,
            "hash": distribution(&hash_samples),
            "verify": distribution(&verify_samples),
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
        let started = Instant::now();
        let mut failures = 0_u32;
        let mut samples = Vec::new();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..count)
                .map(|_| {
                    let reference = stored.clone();
                    scope.spawn(move || {
                        let mut local_failures = 0_u32;
                        let mut local_samples = Vec::with_capacity(rounds as usize);
                        for _ in 0..rounds {
                            let op_started = Instant::now();
                            match verify_credential(&VerifyRequest {
                                purpose: CredentialKind::Password,
                                algorithm: CredentialAlgorithm::Argon2id,
                                stored: &reference,
                                password: PASSWORD,
                            }) {
                                Ok(VerificationOutcome::Verified { .. }) => {
                                    local_samples.push(millis(op_started.elapsed()));
                                }
                                _ => local_failures += 1,
                            }
                        }
                        (local_failures, local_samples)
                    })
                })
                .collect();
            for handle in handles {
                match handle.join() {
                    Ok((local_failures, local_samples)) => {
                        failures += local_failures;
                        samples.extend(local_samples);
                    }
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
            "wall_ms": millis(elapsed),
            "throughput_per_s": throughput(operations, elapsed),
            "latency": distribution(&samples),
            "failures": failures,
        }));
    }
    Ok(Value::Array(results))
}

/// Ráfaga corta: muchos intentos a la vez, como una avalancha de credenciales.
///
/// No aplica límites: mide cuánto tarda el sistema en atender la ráfaga y si
/// alguna verificación falla por contención.
fn measure_burst(workers: u32) -> Result<Value, Box<dyn std::error::Error>> {
    let policy = Argon2Policy::LAB_V1;
    let stored = hash_password(PASSWORD, policy)?;
    let started = Instant::now();
    let mut failures = 0_u32;
    let mut samples = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let reference = stored.clone();
                scope.spawn(move || {
                    let op_started = Instant::now();
                    let outcome = verify_credential(&VerifyRequest {
                        purpose: CredentialKind::Password,
                        algorithm: CredentialAlgorithm::Argon2id,
                        stored: &reference,
                        password: PASSWORD,
                    });
                    match outcome {
                        Ok(VerificationOutcome::Verified { .. }) => {
                            (0_u32, Some(millis(op_started.elapsed())))
                        }
                        _ => (1_u32, None),
                    }
                })
            })
            .collect();
        for handle in handles {
            match handle.join() {
                Ok((local_failures, sample)) => {
                    failures += local_failures;
                    if let Some(value) = sample {
                        samples.push(value);
                    }
                }
                Err(_) => failures += 1,
            }
        }
    });
    let elapsed = started.elapsed();
    Ok(json!({
        "policy": policy.id(),
        "simultaneous_attempts": workers,
        "wall_ms": millis(elapsed),
        "throughput_per_s": throughput(u64::from(workers), elapsed),
        "latency": distribution(&samples),
        "failures": failures,
        "note": "Una ráfaga sin límites mide contención; el throttling por usuario/IP es política de aplicación, no criptográfica.",
    }))
}

/// Distribución de latencias en milisegundos, incluidos percentiles.
fn distribution(samples: &[f64]) -> Value {
    if samples.is_empty() {
        return json!({ "count": 0 });
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let count = sorted.len();
    json!({
        "count": count,
        "min_ms": sorted[0],
        "p50_ms": percentile(&sorted, 50.0),
        "p95_ms": percentile(&sorted, 95.0),
        "p99_ms": percentile(&sorted, 99.0),
        "max_ms": sorted[count - 1],
        "mean_ms": sorted.iter().sum::<f64>() / f64::from(u32::try_from(count).unwrap_or(1)),
    })
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let last = sorted.len().saturating_sub(1);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let index = ((percentile / 100.0) * last as f64).round() as usize;
    sorted[index.min(last)]
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
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

/// Mantiene documentada la longitud de sal exigida por el contrato PHC.
const _: usize = SALT_BYTES;
