//! Frontera explícita para plataformas sin sockets Unix y permisos POSIX.
//!
//! El laboratorio PostgreSQL crea un cluster nativo aislado con socket Unix y
//! directorios `0700`. Windows no ofrece una equivalencia de esas garantías por
//! esta ruta, por lo que compila el workspace pero rechaza ejecutar el lab en
//! lugar de degradarlo a TCP o de declarar checks aprobados.

use std::path::Path;

use serde_json::Value;

use crate::{LabError, Result, report::Evidence};

/// Rechaza el laboratorio PostgreSQL nativo fuera de Unix.
///
/// Ejecútese en Linux o WSL2 con los binarios PostgreSQL 17 construidos por el
/// script del proyecto. El llamador registra el resultado como `FAILED` técnico,
/// nunca como `PASSED` o `NOT_REQUESTED` cuando se solicitó explícitamente.
pub(crate) async fn run(_bin: &Path, _evidence: &Evidence) -> Result<Value> {
    Err(LabError::Invalid(
        "el laboratorio PostgreSQL nativo requiere Unix/Linux o WSL2; Windows no emula sockets Unix ni permisos POSIX"
            .into(),
    ))
}
