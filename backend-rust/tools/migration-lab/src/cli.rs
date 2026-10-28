//! Argumentos del laboratorio; PostgreSQL es nativo y no se aceptan URLs.

use std::{ffi::OsString, path::PathBuf};

use crate::{LabError, Result};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Args {
    pub output: PathBuf,
    pub require_complete: bool,
    /// Directorio de binarios PostgreSQL 17; si se omite, su evidencia queda NOT_REQUESTED.
    pub postgres_bin: Option<PathBuf>,
    /// Exige además el origen Java/Flyway real; requiere `--postgres-bin`.
    pub java_source: bool,
}

impl Args {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Option<Self>> {
        let mut args = args.into_iter();
        let mut output = None;
        let mut require_complete = false;
        let mut postgres_bin = None;
        let mut java_source = false;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--help" | "-h")
                    if output.is_none()
                        && !require_complete
                        && postgres_bin.is_none()
                        && !java_source =>
                {
                    if args.next().is_some() {
                        return Err(LabError::Invalid(
                            "--help no admite otros argumentos".into(),
                        ));
                    }
                    return Ok(None);
                }
                Some("--output") if output.is_none() => {
                    let value = args.next().ok_or_else(|| {
                        LabError::Invalid("falta el directorio de --output".into())
                    })?;
                    if value.is_empty() || value.to_string_lossy().starts_with('-') {
                        return Err(LabError::Invalid("--output requiere una ruta".into()));
                    }
                    output = Some(PathBuf::from(value));
                }
                Some("--postgres-bin") if postgres_bin.is_none() => {
                    let value = args.next().ok_or_else(|| {
                        LabError::Invalid("falta el directorio de --postgres-bin".into())
                    })?;
                    if value.is_empty() || value.to_string_lossy().starts_with('-') {
                        return Err(LabError::Invalid(
                            "--postgres-bin requiere un directorio".into(),
                        ));
                    }
                    postgres_bin = Some(PathBuf::from(value));
                }
                Some("--java-source") if !java_source => java_source = true,
                Some("--require-complete") if !require_complete => require_complete = true,
                _ => return Err(LabError::Invalid("argumento desconocido o repetido".into())),
            }
        }
        if java_source && postgres_bin.is_none() {
            return Err(LabError::Invalid(
                "--java-source exige --postgres-bin para el origen Flyway".into(),
            ));
        }
        Ok(Some(Self {
            output: output.ok_or_else(|| LabError::Invalid("se requiere --output".into()))?,
            require_complete,
            postgres_bin,
            java_source,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Args>> {
        Args::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_output_strict_gate_and_optional_native_labs() -> Result<()> {
        let expected = Some(Args {
            output: "target/evidence".into(),
            require_complete: true,
            postgres_bin: Some("/opt/pg17/bin".into()),
            java_source: true,
        });
        assert_eq!(
            parse(&[
                "--output",
                "target/evidence",
                "--postgres-bin",
                "/opt/pg17/bin",
                "--java-source",
                "--require-complete",
            ])?,
            expected
        );
        Ok(())
    }

    #[test]
    fn rejects_missing_unknown_duplicate_and_source_without_postgres() {
        for args in [
            vec![],
            vec!["--output"],
            vec!["--output", ""],
            vec!["--output", "--require-complete"],
            vec!["--database-url", "postgres://example"],
            vec!["--output", "a", "--output", "b"],
            vec!["--output", "a", "--require-complete", "--require-complete"],
            vec!["--help", "--output", "a"],
            vec!["--output", "a", "--java-source"],
            vec!["--output", "a", "--postgres-bin"],
            vec![
                "--output",
                "a",
                "--postgres-bin",
                "x",
                "--postgres-bin",
                "y",
            ],
            vec!["--output", "a", "--java-source", "--java-source"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn help_does_not_require_an_output() -> Result<()> {
        assert_eq!(parse(&["--help"])?, None);
        Ok(())
    }
}
