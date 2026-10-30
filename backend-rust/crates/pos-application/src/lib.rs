//! Capa de aplicación del monolito modular Local.
//!
//! Contiene coordinación de casos de uso y puertos de salida **independientes de
//! SQLx**. No conoce tablas, pools ni transacciones concretas: los adaptadores
//! implementan los traits declarados aquí sobre la misma conexión física.
//!
//! # Reglas de dependencia
//!
//! ```text
//! pos-core  <-  pos-application  <-  pos-storage-sqlite  <-  pos-local
//! ```
//!
//! - `pos-core` no conoce esta capa.
//! - Esta capa no depende de `sqlx`, `tokio`, HTTP ni Tauri.
//! - Los puertos se definen aquí porque solo la aplicación sabe qué necesita.
//!
//! # Alcance actual
//!
//! La extracción es incremental y verificable: primero se consolidan las
//! decisiones de autorización, atribución de auditoría y contexto operativo que
//! antes estaban duplicadas en cada adaptador SQLite. Los casos de uso completos
//! de catálogo, caja, ventas e inventario se moverán aquí en incrementos
//! posteriores, preservando los contratos y las pruebas existentes.

/// Construcción de eventos auditables y puerto de escritura transaccional.
pub mod audit;
/// Comprobaciones de scope y permiso previas a cualquier caso de uso.
pub mod authorization;
/// Casos de uso, DTOs y puertos de cajas y turnos.
pub mod cash;
/// Caso de uso de checkout atómico.
pub mod checkout;
/// Contexto operativo de una venta (sucursal, caja y turno vigentes).
pub mod sales;
pub mod sales_context;

/// Resultado de una actualización condicional por revisión (CAS).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasOutcome {
    /// La fila se actualizó y la versión ya avanzó en el dominio.
    Updated,
    /// La fila no existe en el scope solicitado.
    Missing,
    /// La fila existe pero la revisión previa ya no coincide.
    Conflict,
}

#[cfg(test)]
mod architecture_tests;
