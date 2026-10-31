//! Exclusión entre procesos y recuperación tras terminación forzada.
use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn holds_installation_lock_in_child() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::var_os("HIQNET_TEST_LOCK_DIRECTORY") else {
        return Ok(());
    };
    let directory = InstallationDirectory::resolve(Path::new(&path))?;
    let _guard = directory.acquire_lock()?;
    fs::write(Path::new(&path).join("ready"), b"ready")?;
    // El padre debe terminar este proceso. El techo también evita un hijo
    // indefinido si falla la prueba principal antes de llamar a kill.
    std::thread::sleep(Duration::from_secs(10));
    Ok(())
}

#[test]
fn lock_is_recovered_after_abrupt_process_termination() -> Result<(), Box<dyn std::error::Error>> {
    let parent = tempfile::tempdir()?;
    let path = parent.path().join("installation");
    let directory = InstallationDirectory::resolve(&path)?;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("holds_installation_lock_in_child")
        .env("HIQNET_TEST_LOCK_DIRECTORY", &path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.join("ready").exists() && Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Err("el proceso de prueba terminó antes de adquirir el candado".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = path.join("ready").exists();
    let rejected = matches!(directory.acquire_lock(), Err(DirectoryError::InUse));
    child.kill()?;
    child.wait()?;
    assert!(ready, "el hijo no adquirió el candado a tiempo");
    assert!(rejected, "otro proceso pudo abrir la instalación bloqueada");
    let recovered = directory.acquire_lock()?;
    drop(recovered);
    assert!(path.join(LOCK_FILE_NAME).is_file());
    Ok(())
}
