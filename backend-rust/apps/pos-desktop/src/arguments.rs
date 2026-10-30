//! Análisis de argumentos: modos explícitos y sin valores por defecto de producto.
//!
//! No se interpretan variables de entorno para elegir el modo ni la ruta: un
//! arranque ambiguo se rechaza en lugar de adivinar.

use std::path::PathBuf;

/// Acción solicitada al ejecutable.
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    /// Exporta los bindings TypeScript desde el registro real.
    ExportBindings,
    /// Laboratorio sintético con base efímera.
    SyntheticLab,
    /// Instalación persistente en el directorio indicado.
    Installation(PathBuf),
}

/// Fallos de uso; el mensaje no reproduce los argumentos recibidos.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentError {
    /// No se indicó ningún modo.
    MissingMode,
    /// Se indicaron dos modos incompatibles.
    ConflictingModes,
    /// `--installation` sin directorio.
    MissingInstallationDirectory,
    /// Directorio de instalación vacío.
    EmptyInstallationDirectory,
    /// Argumento desconocido.
    UnknownArgument,
}

/// Interpreta los argumentos posteriores al nombre del ejecutable.
///
/// # Errors
/// Rechaza combinaciones ambiguas y valores ausentes.
pub fn parse<I, S>(arguments: I) -> Result<Invocation, ArgumentError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut invocation = None;
    let mut arguments = arguments.into_iter().map(Into::into);
    while let Some(argument) = arguments.next() {
        let selected = match argument.as_str() {
            "--export-bindings" => Invocation::ExportBindings,
            "--synthetic-lab" => Invocation::SyntheticLab,
            "--installation" => {
                let directory = arguments
                    .next()
                    .ok_or(ArgumentError::MissingInstallationDirectory)?;
                if directory.is_empty() {
                    return Err(ArgumentError::EmptyInstallationDirectory);
                }
                Invocation::Installation(PathBuf::from(directory))
            }
            _ => return Err(ArgumentError::UnknownArgument),
        };
        if invocation.is_some() {
            return Err(ArgumentError::ConflictingModes);
        }
        invocation = Some(selected);
    }
    invocation.ok_or(ArgumentError::MissingMode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_explicit_modes() {
        assert_eq!(parse(["--export-bindings"]), Ok(Invocation::ExportBindings));
        assert_eq!(parse(["--synthetic-lab"]), Ok(Invocation::SyntheticLab));
        assert_eq!(
            parse(["--installation", "C:/datos/hiqnet"]),
            Ok(Invocation::Installation(PathBuf::from("C:/datos/hiqnet")))
        );
    }

    #[test]
    fn rejects_ambiguous_or_incomplete_arguments() {
        assert_eq!(parse(Vec::<String>::new()), Err(ArgumentError::MissingMode));
        assert_eq!(
            parse(["--synthetic-lab", "--installation", "datos"]),
            Err(ArgumentError::ConflictingModes)
        );
        assert_eq!(
            parse(["--installation"]),
            Err(ArgumentError::MissingInstallationDirectory)
        );
        assert_eq!(
            parse(["--installation", ""]),
            Err(ArgumentError::EmptyInstallationDirectory)
        );
        assert_eq!(
            parse(["--synthetic-lab", "--unknown"]),
            Err(ArgumentError::UnknownArgument)
        );
    }
}
