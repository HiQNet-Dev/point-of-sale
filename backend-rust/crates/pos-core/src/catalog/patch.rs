/// Campo de patch que distingue ausencia, limpieza explícita y valor nuevo.
///
/// Replica el contrato `UuidPatchField` de Java: un campo ausente no se toca y
/// un `null` explícito limpia el valor opcional.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PatchField<T> {
    /// No se envía: el valor actual se conserva.
    #[default]
    Absent,
    /// Se envía `null`: el valor opcional se limpia.
    Clear,
    /// Se envía un valor nuevo.
    Set(T),
}
