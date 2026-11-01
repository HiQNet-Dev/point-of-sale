//! Administración del negocio: configuración de la organización y de su sucursal.
//!
//! # Por qué una composición asíncrona propia
//!
//! [`super::OrganizationApplication`] y [`super::BranchApplication`] son la
//! composición síncrona sobre puertos (`UnitOfWork::transaction(fn)`). No puede
//! sostener la frontera transaccional productiva porque
//! `SqliteUnitOfWork::begin` es asíncrona y no cabe dentro de un cierre síncrono
//! sin bloquear el runtime; además audita con la forma previa a la atribución
//! canónica USER/SYSTEM, que la tabla `audit_event` no admite.
//!
//! Esta composición reutiliza **las mismas reglas de dominio** (`Organization` y
//! `Branch` con sus patches, CAS y listas de campos cambiados) sobre los puertos
//! asíncronos que implementa `SqliteUnitOfWork`, igual que el catálogo. No
//! duplica ninguna invariante: el dominio decide qué cambia y qué revisión
//! resulta.
//!
//! # Alcance de edición de Free Local v1
//!
//! | Campo | Estado | Motivo |
//! | --- | --- | --- |
//! | `organization.name` | editable | Campo visible sin efecto operativo |
//! | `organization.timezone` | editable | Base de la herencia de sucursal |
//! | `organization.locale` | editable | Campo visible sin efecto operativo |
//! | `organization.stock_negative_policy` | editable | Contrato aprobado en ADR-0013 |
//! | `organization.status` | solo lectura | Sin contrato de baja ni recuperación |
//! | `organization.currency` | solo lectura | Reinterpretaría importes persistidos |
//! | `branch.name` | editable | Campo visible sin efecto operativo |
//! | `branch.timezone_override` | editable | Triestado del dominio |
//! | `branch.status` | solo lectura | Desactivar la única sucursal dejaría la instalación inoperable |
//!
//! Un campo de solo lectura no existe en el contrato de entrada: al rechazar
//! campos desconocidos, un renderer comprometido tampoco puede enviarlo.
//!
//! # Capacidad de sucursales
//!
//! Free Local v1 opera con una sola sucursal. La regla se aplica en esta capa
//! ([`OrganizationAdministrationApplication::create_branch`]): la primera
//! sucursal se admite y cualquier intento posterior responde
//! `BranchCapacityReached`, aunque el llamador tenga `branch.create`. Ocultar un
//! botón no es la garantía; la política es del producto y vive en Rust.

mod dto;
mod ports;
mod use_cases;

pub use dto::{
    BranchConfiguration, BranchConfigurationUpdate, BranchCreation,
    OrganizationConfigurationUpdate, TimezoneUpdate,
};
pub use ports::{OrganizationAdministrationTransaction, OrganizationAdministrationUnitOfWork};
pub use use_cases::OrganizationAdministrationApplication;
