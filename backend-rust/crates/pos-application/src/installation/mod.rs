//! Ciclo de vida de la instalación Local persistente.
//!
//! Este módulo **no** interpreta el esquema ni activa DDL: solo clasifica el
//! historial de migraciones aplicado por el migrador autorizado y detecta bases
//! que el binario actual no puede abrir con garantías.
//!
//! La política es deliberadamente pequeña y comprobable:
//!
//! - Un historial vacío es una **primera instalación** y exige migrar antes de
//!   operar.
//! - Un historial con la versión aprobada por este build es una instalación
//!   **reabierta**.
//! - Un historial por delante de la versión aprobada pertenece a un binario más
//!   reciente y se rechaza: operar sobre un esquema desconocido rompería
//!   invariantes antes de que la aplicación pueda detectarlo.
//!
//! El contenido del esquema es responsabilidad de `migrations/local`, que en
//! v0.4.0 publica `0001`–`0017`.

use std::fmt;

/// Última versión de migración que este build puede operar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovedSchema {
    version: i64,
}

impl ApprovedSchema {
    /// Declara la versión aprobada explícitamente; el composition root la deriva
    /// del migrador embebido para que no pueda desincronizarse.
    ///
    /// # Errors
    /// Devuelve [`InstallationError::InvalidApprovedSchema`] si la versión es
    /// negativa, ya que ese valor no corresponde a ninguna migración SQLx.
    pub fn new(version: i64) -> Result<Self, InstallationError> {
        if version < 0 {
            return Err(InstallationError::InvalidApprovedSchema);
        }
        Ok(Self { version })
    }

    /// Versión aprobada por el build.
    pub fn version(self) -> i64 {
        self.version
    }
}

/// Hecho observado por el adaptador sobre el archivo y su historial SQLx.
///
/// No contiene rutas, SQL ni valores de filas: solo el estado necesario para
/// clasificar la instalación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaObservation {
    /// El archivo ya existía antes de abrirlo, es decir, no lo creó este proceso.
    pub database_existed: bool,
    /// Migraciones con `success = 1` aplicadas por SQLx.
    pub applied_migrations: u32,
    /// Versión máxima aplicada, `None` cuando el historial está vacío.
    pub latest_version: Option<i64>,
}

/// Estado clasificado de la instalación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallationState {
    /// No había migraciones al abrir: este arranque debía migrar antes de operar.
    FirstInstall,
    /// El historial ya cumplía la versión aprobada al abrir.
    Reopened,
}

/// Resultado del caso de uso de preparación de instalación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstallationReport {
    /// Clasificación obtenida **al abrir**, antes de cualquier migración.
    pub state: InstallationState,
    /// Versión de esquema efectiva al finalizar la preparación; `0` = historial vacío.
    pub schema_version: i64,
    /// Migraciones aplicadas con éxito al finalizar la preparación.
    pub applied_migrations: u32,
    /// El archivo preexistía antes del arranque actual.
    pub database_existed: bool,
}

impl InstallationReport {
    /// Indica si la primera instalación incorporó migraciones.
    ///
    /// No indica si se invocó el migrador: también se ejecuta al reabrir para
    /// validar checksums y aplicar upgrades aprobados pendientes.
    pub fn was_migrated(self) -> bool {
        self.state == InstallationState::FirstInstall && self.applied_migrations > 0
    }
}

/// Puerto de lectura del historial de migraciones.
///
/// El adaptador conoce el archivo concreto; la aplicación solo consume el hecho
/// observado. No se declara como `dyn` porque cada composition root usa un único
/// adaptador real.
#[allow(async_fn_in_trait)]
pub trait InstallationReader {
    /// Observa el archivo y su historial sin modificarlos.
    ///
    /// # Errors
    /// Devuelve [`InstallationError::Unavailable`] si el historial no puede
    /// leerse; el detalle del adaptador no se propaga.
    async fn observe(&self) -> Result<SchemaObservation, InstallationError>;
}

/// Puerto del migrador autorizado.
///
/// Solo puede invocarlo esta capa; la aplicación decide *cuándo* migrar, no
/// *cómo*. Ningún esquema candidato entra por aquí: el adaptador aplica el root
/// aprobado embebido.
#[allow(async_fn_in_trait)]
pub trait AuthorizedMigrator {
    /// Aplica las migraciones aprobadas pendientes.
    ///
    /// # Errors
    /// Devuelve [`InstallationError::MigrationFailed`] sin exponer SQL.
    async fn apply_approved_migrations(&self) -> Result<(), InstallationError>;
}

/// Clasifica una observación conforme a la política de instalación.
///
/// # Errors
/// Rechaza historiales incompletos y esquemas por delante del build actual.
pub fn classify(
    observation: SchemaObservation,
    approved: ApprovedSchema,
) -> Result<InstallationReport, InstallationError> {
    let report = |state, applied_migrations| InstallationReport {
        state,
        schema_version: observation.latest_version.unwrap_or(0),
        applied_migrations,
        database_existed: observation.database_existed,
    };
    if observation.applied_migrations == 0 {
        // Un archivo recién creado y un archivo preexistente sin historial son la
        // misma situación operativa: todavía no hay esquema que operar.
        return Ok(report(InstallationState::FirstInstall, 0));
    }
    let applied = observation
        .latest_version
        .ok_or(InstallationError::InconsistentHistory)?;
    if applied > approved.version() {
        return Err(InstallationError::SchemaAhead {
            found: applied,
            approved: approved.version(),
        });
    }
    Ok(report(
        InstallationState::Reopened,
        observation.applied_migrations,
    ))
}

/// Prepara la instalación: rechaza esquemas futuros, valida el historial y aplica
/// únicamente las migraciones aprobadas pendientes antes de volver a observar.
///
/// El migrador también se invoca al reabrir: SQLx debe comprobar los checksums
/// aunque no haya migraciones pendientes. No equivale a reaplicar el DDL.
///
/// La clasificación de [`InstallationReport::state`] corresponde al momento de
/// apertura, de modo que un arranque que creó el esquema se informa como
/// [`InstallationState::FirstInstall`] aunque después ya tenga migraciones.
///
/// # Errors
/// Propaga los fallos de lectura y de migración, y rechaza esquemas posteriores
/// al build actual.
pub async fn prepare_installation<R: InstallationReader, M: AuthorizedMigrator>(
    reader: &R,
    migrator: &M,
    approved: ApprovedSchema,
) -> Result<InstallationReport, InstallationError> {
    let initial = classify(reader.observe().await?, approved)?;
    migrator.apply_approved_migrations().await?;
    let settled = classify(reader.observe().await?, approved)?;
    if settled.schema_version != approved.version() {
        return Err(InstallationError::InconsistentHistory);
    }
    Ok(InstallationReport {
        state: initial.state,
        ..settled
    })
}

/// Fallos del ciclo de vida de instalación.
///
/// Ninguna variante transporta rutas, SQL ni mensajes del motor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallationError {
    /// La versión aprobada declarada no es válida.
    InvalidApprovedSchema,
    /// El historial declara migraciones pero no una versión máxima.
    InconsistentHistory,
    /// La base fue creada por un binario más reciente.
    SchemaAhead {
        /// Versión encontrada en el archivo.
        found: i64,
        /// Versión que este build puede operar.
        approved: i64,
    },
    /// El adaptador no pudo leer el historial.
    Unavailable,
    /// El migrador autorizado rechazó el esquema.
    MigrationFailed,
}

impl fmt::Display for InstallationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidApprovedSchema => {
                formatter.write_str("versión de esquema aprobada inválida")
            }
            Self::InconsistentHistory => {
                formatter.write_str("historial de migraciones incoherente")
            }
            Self::SchemaAhead { .. } => {
                formatter.write_str("la base requiere un binario más reciente")
            }
            Self::Unavailable => formatter.write_str("estado de instalación no disponible"),
            Self::MigrationFailed => formatter.write_str("migración aprobada rechazada"),
        }
    }
}

impl std::error::Error for InstallationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(database_existed: bool, applied: u32, latest: Option<i64>) -> SchemaObservation {
        SchemaObservation {
            database_existed,
            applied_migrations: applied,
            latest_version: latest,
        }
    }

    fn approved() -> ApprovedSchema {
        ApprovedSchema::new(1).unwrap_or(ApprovedSchema { version: 0 })
    }

    #[test]
    fn empty_history_is_a_first_install() {
        assert_eq!(
            classify(observation(false, 0, None), approved()),
            Ok(InstallationReport {
                state: InstallationState::FirstInstall,
                schema_version: 0,
                applied_migrations: 0,
                database_existed: false,
            })
        );
        let existing_empty = classify(observation(true, 0, None), approved());
        assert!(matches!(
            existing_empty,
            Ok(InstallationReport {
                state: InstallationState::FirstInstall,
                ..
            })
        ));
    }

    #[test]
    fn applied_baseline_is_reopened() {
        let report = classify(observation(true, 1, Some(1)), approved());
        assert_eq!(
            report,
            Ok(InstallationReport {
                state: InstallationState::Reopened,
                schema_version: 1,
                applied_migrations: 1,
                database_existed: true,
            })
        );
        assert!(report.is_ok_and(|value| !value.was_migrated()));
    }

    #[test]
    fn newer_schema_is_rejected() {
        assert_eq!(
            classify(observation(true, 3, Some(2)), approved()),
            Err(InstallationError::SchemaAhead {
                found: 2,
                approved: 1
            })
        );
    }

    #[test]
    fn inconsistent_history_is_rejected() {
        assert_eq!(
            classify(observation(true, 2, None), approved()),
            Err(InstallationError::InconsistentHistory)
        );
        assert_eq!(
            ApprovedSchema::new(-1),
            Err(InstallationError::InvalidApprovedSchema)
        );
    }

    #[tokio::test]
    async fn pending_approved_upgrade_is_applied() -> Result<(), InstallationError> {
        let fixture = Fixture {
            applied: std::cell::Cell::new(Some(0)),
            existed: true,
            fail: false,
        };
        let report = prepare_installation(&fixture, &fixture, approved()).await?;
        assert_eq!(report.state, InstallationState::Reopened);
        assert_eq!(report.schema_version, 1);
        Ok(())
    }

    #[tokio::test]
    async fn migrator_must_reach_the_approved_version() -> Result<(), InstallationError> {
        let fixture = Fixture {
            applied: std::cell::Cell::new(Some(1)),
            existed: true,
            fail: false,
        };
        assert_eq!(
            prepare_installation(&fixture, &fixture, ApprovedSchema::new(2)?).await,
            Err(InstallationError::InconsistentHistory)
        );
        Ok(())
    }

    #[tokio::test]
    async fn future_schema_is_rejected_before_migrating() {
        let fixture = Fixture {
            applied: std::cell::Cell::new(Some(99)),
            existed: true,
            fail: false,
        };
        assert!(matches!(
            prepare_installation(&fixture, &RejectingMigrator, approved()).await,
            Err(InstallationError::SchemaAhead { found: 99, .. })
        ));
    }

    /// Adaptador de prueba: simula un archivo que empieza vacío.
    struct Fixture {
        applied: std::cell::Cell<Option<i64>>,
        existed: bool,
        fail: bool,
    }

    impl InstallationReader for Fixture {
        async fn observe(&self) -> Result<SchemaObservation, InstallationError> {
            if self.fail {
                return Err(InstallationError::Unavailable);
            }
            Ok(match self.applied.get() {
                Some(version) => SchemaObservation {
                    database_existed: self.existed,
                    applied_migrations: 1,
                    latest_version: Some(version),
                },
                None => SchemaObservation {
                    database_existed: self.existed,
                    applied_migrations: 0,
                    latest_version: None,
                },
            })
        }
    }

    impl AuthorizedMigrator for Fixture {
        async fn apply_approved_migrations(&self) -> Result<(), InstallationError> {
            self.applied.set(Some(1));
            Ok(())
        }
    }

    #[tokio::test]
    async fn first_install_migrates_once_and_reports_first_install() {
        let fixture = Fixture {
            applied: std::cell::Cell::new(None),
            existed: false,
            fail: false,
        };
        let report = prepare_installation(&fixture, &fixture, approved()).await;
        assert_eq!(
            report,
            Ok(InstallationReport {
                state: InstallationState::FirstInstall,
                schema_version: 1,
                applied_migrations: 1,
                database_existed: false,
            })
        );
        assert!(report.is_ok_and(InstallationReport::was_migrated));
    }

    #[tokio::test]
    async fn reopen_validates_migration_history() {
        let fixture = Fixture {
            applied: std::cell::Cell::new(Some(1)),
            existed: true,
            fail: false,
        };
        assert_eq!(
            prepare_installation(&fixture, &RejectingMigrator, approved()).await,
            Err(InstallationError::MigrationFailed)
        );
    }

    /// Migrador que siempre falla: la reapertura debe propagar su rechazo.
    struct RejectingMigrator;

    impl AuthorizedMigrator for RejectingMigrator {
        async fn apply_approved_migrations(&self) -> Result<(), InstallationError> {
            Err(InstallationError::MigrationFailed)
        }
    }

    #[tokio::test]
    async fn reader_failures_are_propagated() {
        struct Failing;
        impl InstallationReader for Failing {
            async fn observe(&self) -> Result<SchemaObservation, InstallationError> {
                Err(InstallationError::Unavailable)
            }
        }
        assert_eq!(
            prepare_installation(&Failing, &RejectingMigrator, approved()).await,
            Err(InstallationError::Unavailable)
        );
    }

    #[tokio::test]
    async fn migration_failures_are_propagated() {
        let fixture = Fixture {
            applied: std::cell::Cell::new(None),
            existed: false,
            fail: false,
        };
        assert_eq!(
            prepare_installation(&fixture, &RejectingMigrator, approved()).await,
            Err(InstallationError::MigrationFailed)
        );
    }
}
