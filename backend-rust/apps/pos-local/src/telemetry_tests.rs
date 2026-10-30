use super::{Event, emit, subscriber};
use pos_application::installation::{InstallationError, InstallationReport, InstallationState};
use pos_local::config::ConfigError;
use std::{
    io,
    io::Write,
    sync::{Arc, Mutex},
};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut output = self
            .0
            .lock()
            .map_err(|_| io::Error::other("captura bloqueada"))?;
        output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for Captured {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn json_events_exclude_dependency_sql_paths_and_raw_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let captured = Captured::default();
    tracing::subscriber::with_default(subscriber(captured.clone()), || {
        emit(Event::Ready);
        emit(Event::ConfigurationRejected(ConfigError::InvalidDbPath));
        tracing::error!(target: "sqlx::query", sql = "SELECT sensitive_column", "sensitive SQL");
        tracing::error!(target: "sqlx::sqlite", error = "/sensitive/path.db", "sensitive error");
        tracing::error!(target: "dependency", environment = "sensitive_value", "sensitive environment");
    });
    let output = captured
        .0
        .lock()
        .map_err(|_| io::Error::other("captura bloqueada"))?
        .clone();
    let output = String::from_utf8(output)?;
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|line| line.starts_with('{') && line.ends_with('}'))
    );
    assert!(output.contains("\"event\":\"ready\""));
    assert!(output.contains("\"event\":\"configuration_rejected\""));
    assert!(output.contains("\"reason\":\"invalid_db_path\""));
    assert!(!output.contains("sensitive"));
    Ok(())
}

#[test]
fn installation_events_use_a_closed_vocabulary() -> Result<(), Box<dyn std::error::Error>> {
    let captured = Captured::default();
    tracing::subscriber::with_default(subscriber(captured.clone()), || {
        emit(Event::InstallationResolved(InstallationReport {
            state: InstallationState::FirstInstall,
            schema_version: 1,
            applied_migrations: 1,
            database_existed: false,
        }));
        emit(Event::InstallationRejected(
            InstallationError::SchemaAhead {
                found: 2,
                approved: 1,
            },
        ));
    });
    let output = captured
        .0
        .lock()
        .map_err(|_| io::Error::other("captura bloqueada"))?
        .clone();
    let output = String::from_utf8(output)?;
    assert!(output.contains("\"event\":\"installation_ready\""));
    assert!(output.contains("\"state\":\"first_install\""));
    assert!(output.contains("\"schema_version\":1"));
    assert!(output.contains("\"event\":\"installation_rejected\""));
    assert!(output.contains("\"reason\":\"schema_ahead\""));
    Ok(())
}
