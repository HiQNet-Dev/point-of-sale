//! Regresiones Unix del apagado real, con plazos y limpieza limitada a hijos propios.

#![cfg(unix)]

use std::{
    error::Error,
    fs::{self, File},
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

struct OwnedChild(Child);

impl OwnedChild {
    fn wait_until_exit(&mut self, deadline: Instant) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "el hijo no terminó a tiempo",
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn wait_until_ready(&mut self, log: &Path, deadline: Instant) -> io::Result<()> {
        loop {
            if fs::read_to_string(log)?.contains("\"event\":\"ready\"") {
                return Ok(());
            }
            if self.0.try_wait()?.is_some() {
                return Err(io::Error::other("el hijo terminó antes de ready"));
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "el hijo no llegó a ready",
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn send_signal(&mut self, signal: &str, deadline: Instant) -> io::Result<()> {
        if self.0.try_wait()?.is_some() {
            return Err(io::Error::other(
                "el hijo terminó antes de recibir la señal",
            ));
        }
        // No se recoge al hijo entre esta comprobación y kill: su PID no puede reutilizarse.
        // No usar grupos, nombres de proceso ni PID de fuentes externas.
        let mut sender = Self(
            Command::new("kill")
                .args(["-s", signal, "--"])
                .arg(self.0.id().to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        if !sender.wait_until_exit(deadline)?.success() {
            return Err(io::Error::other("no se pudo enviar la señal al hijo"));
        }
        Ok(())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            // Child::kill actúa únicamente sobre el hijo que conserva este guard.
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn assert_clean_shutdown(signal: &str) -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let log_path = temp.path().join("events.jsonl");
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_pos-local"))
            .env_clear()
            .env("POS_LOCAL_DB_PATH", temp.path().join("local.sqlite"))
            .env("POS_LOCAL_BUSY_TIMEOUT_MS", "100")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(File::create(&log_path)?)
            .spawn()?,
    );
    let result = (|| {
        child.wait_until_ready(&log_path, deadline)?;
        child.send_signal(signal, deadline)?;
        child.wait_until_exit(deadline)
    })();
    // Recoger al hijo antes de leer evidencias o borrar su directorio, incluso ante error.
    drop(child);
    let log = fs::read_to_string(&log_path)?;
    let status =
        result.map_err(|error| io::Error::new(error.kind(), format!("{error}; eventos: {log}")))?;
    assert_eq!(status.code(), Some(0), "señal {signal}; eventos: {log}");
    let ready = log.find("\"event\":\"ready\"").ok_or("falta ready")?;
    let shutdown = log
        .find("\"event\":\"shutdown_requested\"")
        .ok_or("falta shutdown_requested")?;
    let stopped = log.find("\"event\":\"stopped\"").ok_or("falta stopped")?;
    assert!(
        ready < shutdown && shutdown < stopped,
        "eventos fuera de orden: {log}"
    );
    assert!(
        !log.contains("checkpoint_failed"),
        "falló el checkpoint: {log}"
    );
    Ok(())
}

#[test]
fn sigterm_checkpoints_and_exits_successfully() -> Result<(), Box<dyn Error>> {
    assert_clean_shutdown("TERM")
}

#[test]
fn sigint_checkpoints_and_exits_successfully() -> Result<(), Box<dyn Error>> {
    assert_clean_shutdown("INT")
}
