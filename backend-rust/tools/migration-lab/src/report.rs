//! Evidencias explícitas: una prueba sintética nunca acredita paridad normativa.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

use crate::{LabError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum Status {
    Passed,
    Failed,
    Blocked,
    NotRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Coverage {
    SyntheticTechnical,
    NormativeUncovered,
    NormativeExact,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Case {
    pub id: String,
    pub status: Status,
    pub coverage: Coverage,
    pub detail: String,
}

impl Case {
    pub fn technical(id: &str, result: Result<String>) -> Self {
        match result {
            Ok(detail) => Self {
                id: id.into(),
                status: Status::Passed,
                coverage: Coverage::SyntheticTechnical,
                detail,
            },
            Err(error) => Self {
                id: id.into(),
                status: Status::Failed,
                coverage: Coverage::SyntheticTechnical,
                detail: error.to_string(),
            },
        }
    }
}

pub(crate) const OFFLINE_LIMITATION: &str = "El runner no certifica el aislamiento network-denied ni valida el esquema completo; un wrapper externo puede adjuntar una observación técnica sin aprobar DDL13-10.";

pub(crate) fn normative_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (prefix, count) in [("DDL13-", 11), ("PAR-", 9), ("AS-", 10)] {
        for number in 1..=count {
            let id = format!("{prefix}{number:02}");
            let offline = id == "DDL13-10";
            cases.push(Case {
                id,
                status: if offline { Status::NotRun } else { Status::Blocked },
                coverage: Coverage::NormativeUncovered,
                detail: if offline {
                    OFFLINE_LIMITATION
                } else {
                    "Sin cobertura exacta del caso normativo: los roots F1-10 son subconjuntos y lab_parent/lab_child son fixtures sintéticos, no el esquema completo ni paridad Java/PostgreSQL."
                }.into(),
            });
        }
    }
    cases
}

/// Calcula el código de salida sin convertir un bloqueo en una aprobación.
pub(crate) fn exit_code(technical: &[Case], normative: &[Case], require_complete: bool) -> u8 {
    if technical.is_empty() || technical.iter().any(|case| case.status != Status::Passed) {
        return 1;
    }
    let expected: BTreeSet<_> = normative_cases().into_iter().map(|case| case.id).collect();
    let actual: BTreeSet<_> = normative.iter().map(|case| case.id.clone()).collect();
    let complete = actual == expected
        && normative.len() == expected.len()
        && normative
            .iter()
            .all(|case| case.status == Status::Passed && case.coverage == Coverage::NormativeExact);
    if require_complete && !complete { 2 } else { 0 }
}

fn xml_escape(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// JUnit representa BLOCKED/NOT_RUN como skipped, nunca como éxitos vacíos.
pub(crate) fn junit(name: &str, cases: &[Case]) -> String {
    junit_cases(name, &cases.iter().collect::<Vec<_>>())
}

/// Incluye fallos técnicos en el informe principal, sin aprobar los casos normativos omitidos.
pub(crate) fn parity_junit(technical: &[Case], normative: &[Case]) -> String {
    junit_cases(
        "migration-parity",
        &technical.iter().chain(normative).collect::<Vec<_>>(),
    )
}

fn junit_cases(name: &str, cases: &[&Case]) -> String {
    let failures = cases
        .iter()
        .filter(|case| case.status == Status::Failed)
        .count();
    let skipped = cases
        .iter()
        .filter(|case| matches!(case.status, Status::Blocked | Status::NotRun))
        .count();
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"{}\" tests=\"{}\" failures=\"{failures}\" errors=\"0\" skipped=\"{skipped}\">\n",
        xml_escape(name),
        cases.len()
    );
    for case in cases {
        xml.push_str(&format!(
            "  <testcase classname=\"{:?}\" name=\"{}\">",
            case.coverage,
            xml_escape(&case.id)
        ));
        match case.status {
            Status::Passed => xml.push_str(&format!(
                "<system-out>{}</system-out>",
                xml_escape(&case.detail)
            )),
            Status::Failed => xml.push_str(&format!(
                "<failure message=\"FAILED\">{}</failure>",
                xml_escape(&case.detail)
            )),
            Status::Blocked | Status::NotRun => xml.push_str(&format!(
                "<skipped message=\"{:?}\">{}</skipped>",
                case.status,
                xml_escape(&case.detail)
            )),
        }
        xml.push_str("</testcase>\n");
    }
    xml.push_str("</testsuite>\n");
    xml
}

/// Directorio nuevo y exclusivo; cada artefacto también usa create_new.
pub(crate) struct Evidence {
    pub root: PathBuf,
}

impl Evidence {
    pub fn create(path: &Path) -> Result<Self> {
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            || !matches!(path.components().next_back(), Some(Component::Normal(_)))
        {
            return Err(LabError::Invalid(
                "--output debe nombrar un directorio nuevo, sin '..'".into(),
            ));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path
            .file_name()
            .ok_or_else(|| LabError::Invalid("output sin nombre".into()))?;
        let root = parent.canonicalize()?.join(name);
        fs::create_dir(&root)?;
        for subdir in ["schema-snapshots", "failure-logs", "backup-fixtures"] {
            fs::create_dir(root.join(subdir))?;
        }
        Ok(Self { root })
    }

    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
        let path = Path::new(relative);
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(LabError::Invalid(
                "ruta interna de evidencia inválida".into(),
            ));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(path))?;
        file.write_all(bytes.as_ref())?;
        file.sync_all()?;
        Ok(())
    }

    pub fn json(&self, relative: &str, value: &impl Serialize) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        self.write(relative, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normative_identifiers_match_exact_contract() {
        let cases = normative_cases();
        let ids: Vec<_> = cases.iter().map(|case| case.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "DDL13-01", "DDL13-02", "DDL13-03", "DDL13-04", "DDL13-05", "DDL13-06", "DDL13-07",
                "DDL13-08", "DDL13-09", "DDL13-10", "DDL13-11", "PAR-01", "PAR-02", "PAR-03",
                "PAR-04", "PAR-05", "PAR-06", "PAR-07", "PAR-08", "PAR-09", "AS-01", "AS-02",
                "AS-03", "AS-04", "AS-05", "AS-06", "AS-07", "AS-08", "AS-09", "AS-10",
            ]
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.status == Status::Blocked)
                .count(),
            29
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.status == Status::NotRun)
                .count(),
            1
        );
        assert!(
            cases
                .iter()
                .all(|case| case.coverage == Coverage::NormativeUncovered)
        );
    }

    #[test]
    fn normal_run_allows_blocked_normative_but_strict_run_returns_two() {
        let technical = [Case::technical("sqlite", Ok("ejecutado".into()))];
        assert_eq!(exit_code(&technical, &normative_cases(), false), 0);
        assert_eq!(exit_code(&technical, &normative_cases(), true), 2);
        assert_eq!(exit_code(&technical, &[], true), 2);
        assert_eq!(exit_code(&[], &[], false), 1);
    }

    #[test]
    fn synthetic_passes_cannot_complete_normative_gate() {
        let technical = [Case::technical("sqlite", Ok("ejecutado".into()))];
        let mut normative = normative_cases();
        for case in &mut normative {
            case.status = Status::Passed;
            case.coverage = Coverage::SyntheticTechnical;
        }
        assert_eq!(exit_code(&technical, &normative, true), 2);
        for case in &mut normative {
            case.coverage = Coverage::NormativeExact;
        }
        assert_eq!(exit_code(&technical, &normative, true), 0);
        normative[0].id = normative[1].id.clone();
        assert_eq!(exit_code(&technical, &normative, true), 2);
    }

    #[test]
    fn technical_failure_has_priority_over_normative_blocking() {
        let technical = [Case::technical(
            "sqlite",
            Err(LabError::Invalid("falló".into())),
        )];
        assert_eq!(exit_code(&technical, &normative_cases(), false), 1);
        assert_eq!(exit_code(&technical, &normative_cases(), true), 1);
        assert!(junit("technical", &technical).contains("<failure"));
    }

    #[test]
    fn junit_contains_thirty_skips_and_escapes_untrusted_text() {
        let xml = junit("parity<&\"", &normative_cases());
        assert!(xml.contains("tests=\"30\" failures=\"0\" errors=\"0\" skipped=\"30\""));
        assert_eq!(xml.matches("<skipped ").count(), 30);
        assert!(!xml.contains("<system-out>"));
        assert!(xml.contains("parity&lt;&amp;&quot;"));
        assert_eq!(xml_escape("ok\u{0}\u{1}\u{fffe}\u{ffff}<&"), "ok&lt;&amp;");
    }

    #[test]
    fn existing_output_and_existing_artifacts_are_never_overwritten() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("evidence");
        let evidence = Evidence::create(&path)?;
        evidence.write("sentinel", "original")?;
        assert!(Evidence::create(&path).is_err());
        assert!(evidence.write("sentinel", "replacement").is_err());
        assert!(evidence.write("../escape", "escape").is_err());
        assert!(Evidence::create(&path.join("../other")).is_err());
        assert_eq!(fs::read_to_string(path.join("sentinel"))?, "original");
        assert!(!temp.path().join("escape").exists());
        assert!(Evidence::create(&temp.path().join("absent/child")).is_err());
        let existing_file = temp.path().join("existing-file");
        fs::write(&existing_file, "keep")?;
        assert!(Evidence::create(&existing_file).is_err());
        assert_eq!(fs::read_to_string(&existing_file)?, "keep");
        Ok(())
    }
}
