//! DTOs de entrada y paginación de caja.
//!
//! Son tipos de aplicación: no contienen SQL ni conocen el adaptador.

use pos_core::organization_branch::{EntityId, Status};

/// Límite local de lecturas paginadas de cajas y turnos.
pub const MAX_CASH_PAGE_SIZE: u32 = 100;

/// Datos de creación de una caja recibidos desde la frontera de aplicación.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCashRegisterRequest {
    /// Código que se normaliza como slug Java.
    pub code: String,
    /// Nombre visible que se recorta y valida.
    pub name: String,
}

/// Patch CAS de caja. El código y los owners son inmutables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CashRegisterUpdateRequest {
    /// Revisión leída por el cliente.
    pub expected_version: u64,
    /// Nombre opcional.
    pub name: Option<String>,
    /// Estado operativo opcional.
    pub status: Option<Status>,
}

/// Apertura de turno; el actor y el scope se derivan del principal verificado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCashShiftRequest {
    /// Caja dentro de la sucursal scoped.
    pub cash_register_id: EntityId,
    /// Declaración inicial con escala decimal máxima cuatro.
    pub opening_amount: String,
}

/// Cierre no idempotente de turno; el actor se deriva del principal verificado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseCashShiftRequest {
    /// Revisión leída antes de intentar el cierre.
    pub expected_version: u64,
    /// Declaración final con escala decimal máxima cuatro.
    pub closing_declared_amount: String,
}

/// Página determinista de cajas o turnos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CashPage {
    /// Tamaño de página, de 1 a [`MAX_CASH_PAGE_SIZE`].
    pub limit: u32,
    /// Desplazamiento desde el inicio.
    pub offset: u32,
}
