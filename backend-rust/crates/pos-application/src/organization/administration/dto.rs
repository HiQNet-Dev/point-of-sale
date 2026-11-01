//! Contratos de entrada y salida de la administración del negocio.
//!
//! Los campos llegan sin validar, como en el catálogo: el caso de uso aplica las
//! mismas construcciones de dominio que cualquier otra escritura. Un campo
//! ausente significa «conservar»; nunca se resetea a un valor por defecto.

use pos_core::organization_branch::{Branch, Timezone};

/// Cambios solicitados sobre la configuración de la organización.
///
/// No contiene identidad, código, estado ni moneda: el contrato no permite
/// expresar esos cambios ni siquiera desde un cliente comprometido.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrganizationConfigurationUpdate {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre comercial visible.
    pub name: Option<String>,
    /// Zona horaria raíz del negocio.
    pub timezone: Option<String>,
    /// Configuración regional.
    pub locale: Option<String>,
    /// Política de inventario negativo (ADR-0013).
    pub negative_stock_policy: Option<String>,
}

/// Operación triestado sobre la zona horaria de una sucursal.
///
/// Reproduce la semántica de `TimezoneOverridePatch` sin colapsarla en un `null`
/// ambiguo: conservar, sobrescribir y volver a heredar son operaciones
/// distintas.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TimezoneUpdate {
    /// Se conserva la configuración actual.
    #[default]
    Keep,
    /// Se define una zona horaria propia.
    Set(String),
    /// Se elimina el override y la sucursal hereda la de la organización.
    Clear,
}

/// Cambios solicitados sobre una sucursal existente.
///
/// Identidad, código, propietario y estado no forman parte del contrato.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchConfigurationUpdate {
    /// Revisión leída por el llamador.
    pub expected_revision: u64,
    /// Nombre visible.
    pub name: Option<String>,
    /// Operación sobre la zona horaria.
    pub timezone: TimezoneUpdate,
}

/// Datos de alta de una sucursal adicional.
///
/// Free Local no expone esta operación; existe para que la política de capacidad
/// sea aplicable en la capa de aplicación y no dependa de la interfaz.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchCreation {
    /// Código de sucursal, único dentro de la organización.
    pub code: String,
    /// Nombre visible.
    pub name: String,
    /// Zona horaria propia opcional.
    pub timezone: Option<String>,
}

/// Sucursal con la zona horaria efectiva ya resuelta por el dominio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchConfiguration {
    /// Agregado persistido.
    pub branch: Branch,
    /// Zona horaria aplicable, propia o heredada.
    pub effective_timezone: Timezone,
}

impl BranchConfiguration {
    /// Ensambla la vista aplicando la regla de herencia del dominio.
    pub fn new(branch: Branch, organization_timezone: &Timezone) -> Self {
        let effective = branch.effective_timezone(organization_timezone).clone();
        Self {
            branch,
            effective_timezone: effective,
        }
    }

    /// Indica si la sucursal sobrescribe la zona horaria de la organización.
    pub fn overrides_timezone(&self) -> bool {
        self.branch.timezone_override().is_some()
    }
}
