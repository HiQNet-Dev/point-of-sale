use super::{Config, ConfigError};
use std::{ffi::OsString, path::Path, time::Duration};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse(path: Option<&str>, timeout: Option<&str>) -> Result<Config, ConfigError> {
    Config::from_values(path.map(OsString::from), timeout.map(OsString::from))
}

#[test]
fn configuration_requires_both_values_without_product_defaults() -> TestResult {
    assert!(matches!(
        parse(None, Some("1000")),
        Err(ConfigError::MissingDbPath)
    ));
    assert!(matches!(
        parse(Some("local.db"), None),
        Err(ConfigError::MissingBusyTimeout)
    ));
    Ok(())
}

#[test]
fn configuration_accepts_local_file_paths_and_explicit_timeout_limits() -> TestResult {
    for path in [
        "local.db",
        "./data/local.sqlite",
        "/tmp/pos-local.sqlite",
        "data/local file.db",
    ] {
        for timeout in [1, 1000, 60_000] {
            let config = parse(Some(path), Some(&timeout.to_string()))?;
            assert_eq!(config.db_path(), Path::new(path));
            assert_eq!(config.busy_timeout(), Duration::from_millis(timeout));
        }
    }
    Ok(())
}

#[test]
fn configuration_rejects_memory_urls_empty_or_non_file_paths() -> TestResult {
    for path in [
        "",
        " ",
        "\t",
        ":memory:",
        "sqlite::memory:",
        "sqlite:local.db",
        "file:local.db",
        "file:///tmp/local.db",
        "https://example.test/local.db",
        "postgres://localhost/db",
        "data/file:local.db",
        "//host/share/db",
        "\\\\host\\share\\db",
        "data\\db",
        ".",
        "..",
        "/",
        "./",
        "data/..",
        "data/.",
        "data/",
        " db.sqlite",
        "db.sqlite ",
        "local.db?mode=memory",
        "local.db#fragment",
        "local\0.db",
        "local\n.db",
    ] {
        assert!(
            matches!(
                parse(Some(path), Some("1000")),
                Err(ConfigError::InvalidDbPath)
            ),
            "ruta {path:?}"
        );
    }
    Ok(())
}

#[test]
fn configuration_rejects_existing_directories_as_database_files() -> TestResult {
    let current = std::env::current_dir()?;
    assert!(matches!(
        Config::from_values(Some(current.into_os_string()), Some(OsString::from("1000"))),
        Err(ConfigError::InvalidDbPath)
    ));
    Ok(())
}

#[test]
fn configuration_rejects_timeout_outside_explicit_ascii_millisecond_range() -> TestResult {
    for timeout in [
        "",
        "0",
        "60001",
        "-1",
        "+1",
        "1.0",
        " 1",
        "1 ",
        "١",
        "999999999999999999999999",
    ] {
        assert!(
            matches!(
                parse(Some("local.db"), Some(timeout)),
                Err(ConfigError::InvalidBusyTimeout)
            ),
            "timeout {timeout:?}"
        );
    }
    Ok(())
}

#[test]
fn config_error_text_never_echoes_untrusted_input() -> TestResult {
    let result = parse(Some("sqlite://sensitive-path"), Some("1000"));
    let error = result.err().ok_or("se esperaba rechazo de URL")?;
    assert!(!error.to_string().contains("sensitive-path"));
    assert!(!format!("{error:?}").contains("sensitive-path"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn configuration_rejects_non_unicode_environment_values() -> TestResult {
    use std::os::unix::ffi::OsStringExt;
    let invalid = OsString::from_vec(vec![0xff]);
    assert!(matches!(
        Config::from_values(Some(invalid.clone()), Some(OsString::from("1000"))),
        Err(ConfigError::InvalidDbPath)
    ));
    assert!(matches!(
        Config::from_values(Some(OsString::from("local.db")), Some(invalid)),
        Err(ConfigError::InvalidBusyTimeout)
    ));
    Ok(())
}
