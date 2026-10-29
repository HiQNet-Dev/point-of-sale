//! Medición local de Argon2id con políticas explícitas.
//!
//! No es un benchmark normativo ni aprueba parámetros: caracteriza los
//! candidatos de laboratorio en el hardware donde se ejecute. Uso:
//!
//! ```sh
//! cargo run -p pos-credentials --example argon2_bench
//! ```

use std::time::Instant;

use pos_core::identity::{CredentialAlgorithm, CredentialKind};
use pos_credentials::{Argon2Policy, VerifyRequest, hash_password, verify_credential};

const PASSWORD: &str = "bench-only-passphrase";
const ROUNDS: u32 = 5;

fn main() {
    println!(
        "os={} arch={} cpus={} rounds={ROUNDS}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get)
    );
    for policy in [Argon2Policy::LAB_V0, Argon2Policy::LAB_V1] {
        if hash_password(PASSWORD, policy).is_err() {
            println!("policy={} status=ERROR", policy.id());
            continue;
        }
        let start = Instant::now();
        for _ in 0..ROUNDS {
            if hash_password(PASSWORD, policy).is_err() {
                println!("policy={} status=ERROR", policy.id());
                return;
            }
        }
        let hash_ms = start.elapsed().as_secs_f64() * 1_000.0 / f64::from(ROUNDS);

        let stored = match hash_password(PASSWORD, policy) {
            Ok(value) => value,
            Err(_) => {
                println!("policy={} status=ERROR", policy.id());
                return;
            }
        };
        let start = Instant::now();
        for _ in 0..ROUNDS {
            let outcome = verify_credential(&VerifyRequest {
                purpose: CredentialKind::Password,
                algorithm: CredentialAlgorithm::Argon2id,
                stored: &stored,
                password: PASSWORD,
            });
            if outcome.is_err() {
                println!("policy={} status=ERROR", policy.id());
                return;
            }
        }
        let verify_ms = start.elapsed().as_secs_f64() * 1_000.0 / f64::from(ROUNDS);

        println!(
            "policy={} m_kib={} t={} p={} hash_ms={hash_ms:.1} verify_ms={verify_ms:.1} status=OBSERVED",
            policy.id(),
            policy.memory_kib(),
            policy.iterations(),
            policy.lanes()
        );
    }
}
