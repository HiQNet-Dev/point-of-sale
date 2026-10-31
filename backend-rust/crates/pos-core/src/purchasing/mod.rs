//! Dominio puro de compras: órdenes de compra, partidas, recepciones y reglas.
//!
//! Reproduce `purchasing/domain/PurchaseOrder`, `PurchaseOrderLine` y las reglas
//! de `PurchaseReceiptService` sin JPA, Spring, SQLx ni transporte. El agregado
//! `Supplier` vive en [`crate::supplier`]: la orden guarda su identificador y los
//! snapshots de código y nombre, que es lo que el historial necesita.
//!
//! # Ciclo de vida
//!
//! ```text
//! DRAFT --issue--> ORDERED --receipt--> PARTIALLY_RECEIVED --receipt--> RECEIVED
//!   |                  |
//!   +-----cancel-------+--> CANCELLED
//! ```
//!
//! - `issue` solo desde `DRAFT` y es irreversible.
//! - `cancel` se admite desde `DRAFT` u `ORDERED`; conserva los datos de emisión
//!   si la orden ya estaba emitida.
//! - `apply_receipt` solo desde `ORDERED` o `PARTIALLY_RECEIVED`.
//! - El estado nunca se acepta desde el cliente ni se modifica por patch.
//!
//! # Invariantes
//!
//! - Cabecera y partidas comparten la revisión de la cabecera: cualquier mutación
//!   de partida la incrementa.
//! - `code` es un slug inmutable único por organización; la moneda es un snapshot
//!   de la organización y el cliente no puede fijarla.
//! - Cada partida congela código, nombre y unidad del artículo.
//! - La cantidad de una partida es estrictamente positiva; el costo unitario es
//!   opcional y admite cero.
//! - Una recepción nunca puede exceder lo pedido y no admite líneas repetidas.

use std::fmt;

mod receipt;

pub use receipt::{
    NewPurchaseReceipt, PurchaseReceipt, PurchaseReceiptLine, allocate_receipt_line_ids,
};

use crate::{
    Currency,
    catalog::{CatalogCode, UnitPrice},
    identity::UtcSeconds,
    inventory::{InventoryBaseUnit, Quantity},
    organization_branch::{DisplayName, EntityId},
};

/// Fallos de compras sin contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PurchasingError {
    /// La cantidad de una partida o recepción no es positiva.
    InvalidQuantity,
    /// El número de línea debe ser mayor que cero.
    InvalidLineNumber,
    /// La orden no está en `DRAFT` y no admite edición.
    OrderNotDraft,
    /// La orden ya estaba cancelada.
    OrderAlreadyCancelled,
    /// La orden debe emitirse antes de recibir mercancía.
    OrderNotIssued,
    /// La orden ya está completamente recibida.
    OrderAlreadyReceived,
    /// La orden no admite recepciones en su estado actual.
    OrderNotReceivable,
    /// El estado de destino de una recepción no es válido.
    InvalidReceiptStatus,
    /// La recepción repite una línea de la orden.
    DuplicateReceiptLine,
    /// La recepción excede la cantidad pedida.
    ReceiptExceedsOrdered,
    /// La versión esperada no corresponde a la orden.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La entidad referenciada pertenece a otra organización.
    OwnershipMismatch,
    /// La unidad del artículo cambió desde la captura de la partida.
    UnitChanged,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for PurchasingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidQuantity => "cantidad no válida",
            Self::InvalidLineNumber => "número de línea no válido",
            Self::OrderNotDraft => "la orden no está en borrador",
            Self::OrderAlreadyCancelled => "la orden ya está cancelada",
            Self::OrderNotIssued => "la orden debe emitirse antes de recibir",
            Self::OrderAlreadyReceived => "la orden ya está completamente recibida",
            Self::OrderNotReceivable => "la orden no admite recepciones",
            Self::InvalidReceiptStatus => "estado de recepción no válido",
            Self::DuplicateReceiptLine => "la recepción repite una línea",
            Self::ReceiptExceedsOrdered => "la recepción excede la cantidad pedida",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "la entidad pertenece a otra organización",
            Self::UnitChanged => "la unidad del artículo cambió desde la captura",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for PurchasingError {}

/// Estado operativo de una orden de compra.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PurchaseOrderStatus {
    /// Editable; todavía no cursada.
    Draft,
    /// Cursada; admite recepciones.
    Ordered,
    /// Con recepciones parciales.
    PartiallyReceived,
    /// Completamente recibida; estado final.
    Received,
    /// Cancelada; estado final.
    Cancelled,
}

impl PurchaseOrderStatus {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Ordered => "ORDERED",
            Self::PartiallyReceived => "PARTIALLY_RECEIVED",
            Self::Received => "RECEIVED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Convierte un valor persistido sin aceptar estados desconocidos.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, PurchasingError> {
        match value {
            "DRAFT" => Ok(Self::Draft),
            "ORDERED" => Ok(Self::Ordered),
            "PARTIALLY_RECEIVED" => Ok(Self::PartiallyReceived),
            "RECEIVED" => Ok(Self::Received),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(PurchasingError::InvalidPersistedValue),
        }
    }

    /// Indica si la orden admite mercancía en este estado.
    pub fn is_receivable(self) -> bool {
        matches!(self, Self::Ordered | Self::PartiallyReceived)
    }

    /// Indica si la orden es editable.
    pub fn is_draft(self) -> bool {
        self == Self::Draft
    }
}

/// Datos de creación de una orden de compra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPurchaseOrder {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Sucursal de destino, dentro de la misma organización.
    pub branch_id: EntityId,
    /// Proveedor de la organización.
    pub supplier_id: EntityId,
    /// Código comercial ya normalizado; único por organización e inmutable.
    pub code: CatalogCode,
    /// Moneda de la organización copiada como snapshot.
    pub currency: Currency,
    /// Código del proveedor congelado al crear la orden.
    pub supplier_code_snapshot: CatalogCode,
    /// Nombre del proveedor congelado al crear la orden.
    pub supplier_name_snapshot: DisplayName,
    /// Actor autenticado que crea la orden.
    pub created_by_user_id: EntityId,
}

/// Orden de compra propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseOrder {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    supplier_id: EntityId,
    code: CatalogCode,
    status: PurchaseOrderStatus,
    currency: Currency,
    supplier_code_snapshot: CatalogCode,
    supplier_name_snapshot: DisplayName,
    created_by_user_id: EntityId,
    issued_by_user_id: Option<EntityId>,
    cancelled_by_user_id: Option<EntityId>,
    issued_at: Option<UtcSeconds>,
    cancelled_at: Option<UtcSeconds>,
    revision: u64,
}

impl PurchaseOrder {
    /// Crea una orden en `DRAFT` con revisión `0`.
    pub fn create(new: NewPurchaseOrder) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            branch_id: new.branch_id,
            supplier_id: new.supplier_id,
            code: new.code,
            status: PurchaseOrderStatus::Draft,
            currency: new.currency,
            supplier_code_snapshot: new.supplier_code_snapshot,
            supplier_name_snapshot: new.supplier_name_snapshot,
            created_by_user_id: new.created_by_user_id,
            issued_by_user_id: None,
            cancelled_by_user_id: None,
            issued_at: None,
            cancelled_at: None,
            revision: 0,
        }
    }

    /// Rehidrata una orden persistida validando la coherencia de su estado.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::InvalidPersistedValue`] si el estado no
    /// corresponde a los actores e instantes de emisión y cancelación.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        supplier_id: EntityId,
        code: CatalogCode,
        status: PurchaseOrderStatus,
        currency: Currency,
        supplier_code_snapshot: CatalogCode,
        supplier_name_snapshot: DisplayName,
        created_by_user_id: EntityId,
        issued_by_user_id: Option<EntityId>,
        cancelled_by_user_id: Option<EntityId>,
        issued_at: Option<UtcSeconds>,
        cancelled_at: Option<UtcSeconds>,
        revision: u64,
    ) -> Result<Self, PurchasingError> {
        let coherent = match status {
            PurchaseOrderStatus::Draft => {
                issued_at.is_none()
                    && issued_by_user_id.is_none()
                    && cancelled_at.is_none()
                    && cancelled_by_user_id.is_none()
            }
            PurchaseOrderStatus::Ordered | PurchaseOrderStatus::PartiallyReceived => {
                issued_at.is_some()
                    && issued_by_user_id.is_some()
                    && cancelled_at.is_none()
                    && cancelled_by_user_id.is_none()
            }
            PurchaseOrderStatus::Received => {
                issued_at.is_some()
                    && issued_by_user_id.is_some()
                    && cancelled_at.is_none()
                    && cancelled_by_user_id.is_none()
            }
            PurchaseOrderStatus::Cancelled => {
                cancelled_at.is_some() && cancelled_by_user_id.is_some()
            }
        };
        if !coherent {
            return Err(PurchasingError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            supplier_id,
            code,
            status,
            currency,
            supplier_code_snapshot,
            supplier_name_snapshot,
            created_by_user_id,
            issued_by_user_id,
            cancelled_by_user_id,
            issued_at,
            cancelled_at,
            revision,
        })
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal de destino.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }
    /// Proveedor vigente.
    pub fn supplier_id(&self) -> &EntityId {
        &self.supplier_id
    }
    /// Código comercial inmutable.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }
    /// Estado vigente.
    pub fn status(&self) -> PurchaseOrderStatus {
        self.status
    }
    /// Moneda snapshot de la organización.
    pub fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Código del proveedor congelado.
    pub fn supplier_code_snapshot(&self) -> &CatalogCode {
        &self.supplier_code_snapshot
    }
    /// Nombre del proveedor congelado.
    pub fn supplier_name_snapshot(&self) -> &DisplayName {
        &self.supplier_name_snapshot
    }
    /// Actor que creó la orden.
    pub fn created_by_user_id(&self) -> &EntityId {
        &self.created_by_user_id
    }
    /// Actor que emitió la orden, si aplica.
    pub fn issued_by_user_id(&self) -> Option<&EntityId> {
        self.issued_by_user_id.as_ref()
    }
    /// Actor que canceló la orden, si aplica.
    pub fn cancelled_by_user_id(&self) -> Option<&EntityId> {
        self.cancelled_by_user_id.as_ref()
    }
    /// Instante de emisión, si aplica.
    pub fn issued_at(&self) -> Option<UtcSeconds> {
        self.issued_at
    }
    /// Instante de cancelación, si aplica.
    pub fn cancelled_at(&self) -> Option<UtcSeconds> {
        self.cancelled_at
    }
    /// Revisión optimista del agregado.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Exige que la orden esté en `DRAFT` antes de editar cabecera o partidas.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::OrderNotDraft`].
    pub fn require_draft(&self) -> Result<(), PurchasingError> {
        if !self.status.is_draft() {
            return Err(PurchasingError::OrderNotDraft);
        }
        Ok(())
    }

    /// Reasigna el proveedor de un borrador y re-captura sus snapshots.
    ///
    /// Devuelve `false` cuando el proveedor y sus snapshots no cambian. El
    /// javadoc Java declara esa intención aunque su setter siempre toca
    /// `updatedAt`; aquí se implementa la intención declarada para no auditar un
    /// no-op.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::OrderNotDraft`] o
    /// [`PurchasingError::ConcurrentModification`].
    pub fn change_supplier(
        &mut self,
        expected_version: u64,
        supplier_id: EntityId,
        code_snapshot: CatalogCode,
        name_snapshot: DisplayName,
    ) -> Result<bool, PurchasingError> {
        self.require_draft()?;
        if expected_version != self.revision {
            return Err(PurchasingError::ConcurrentModification);
        }
        let unchanged = self.supplier_id == supplier_id
            && self.supplier_code_snapshot == code_snapshot
            && self.supplier_name_snapshot == name_snapshot;
        if unchanged {
            return Ok(false);
        }
        self.supplier_id = supplier_id;
        self.supplier_code_snapshot = code_snapshot;
        self.supplier_name_snapshot = name_snapshot;
        self.bump()?;
        Ok(true)
    }

    /// Registra una mutación de partidas para que la revisión de la cabecera
    /// avance, igual que `registerLineMutation` en Java.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::OrderNotDraft`] o
    /// [`PurchasingError::RevisionOverflow`].
    pub fn register_line_mutation(&mut self) -> Result<(), PurchasingError> {
        self.require_draft()?;
        self.bump()
    }

    /// Emite la orden: transición irreversible `DRAFT → ORDERED`.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::OrderNotDraft`] o
    /// [`PurchasingError::RevisionOverflow`].
    pub fn issue(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), PurchasingError> {
        if !self.status.is_draft() {
            return Err(PurchasingError::OrderNotDraft);
        }
        self.status = PurchaseOrderStatus::Ordered;
        self.issued_by_user_id = Some(actor);
        self.issued_at = Some(at);
        self.bump()
    }

    /// Cancela la orden desde `DRAFT` u `ORDERED`.
    ///
    /// Conserva los datos históricos de emisión cuando la orden ya estaba
    /// emitida.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::OrderAlreadyCancelled`] si ya estaba
    /// cancelada y [`PurchasingError::RevisionOverflow`].
    pub fn cancel(&mut self, actor: EntityId, at: UtcSeconds) -> Result<(), PurchasingError> {
        if self.status == PurchaseOrderStatus::Cancelled {
            return Err(PurchasingError::OrderAlreadyCancelled);
        }
        self.status = PurchaseOrderStatus::Cancelled;
        self.cancelled_by_user_id = Some(actor);
        self.cancelled_at = Some(at);
        self.bump()
    }

    /// Aplica el resultado de una recepción confirmada.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::InvalidReceiptStatus`] si el destino no es
    /// `PARTIALLY_RECEIVED` ni `RECEIVED`, [`PurchasingError::OrderNotReceivable`]
    /// si la orden no admite mercancía y [`PurchasingError::RevisionOverflow`].
    pub fn apply_receipt(&mut self, next: PurchaseOrderStatus) -> Result<(), PurchasingError> {
        if !matches!(
            next,
            PurchaseOrderStatus::PartiallyReceived | PurchaseOrderStatus::Received
        ) {
            return Err(PurchasingError::InvalidReceiptStatus);
        }
        self.ensure_receivable()?;
        self.status = next;
        self.bump()
    }

    /// Comprueba que la orden admita mercancía.
    ///
    /// # Errors
    /// Devuelve el error concreto del estado actual, igual que
    /// `PurchaseReceiptService.requireReceivable`.
    pub fn ensure_receivable(&self) -> Result<(), PurchasingError> {
        match self.status {
            PurchaseOrderStatus::Ordered | PurchaseOrderStatus::PartiallyReceived => Ok(()),
            PurchaseOrderStatus::Draft => Err(PurchasingError::OrderNotIssued),
            PurchaseOrderStatus::Received => Err(PurchasingError::OrderAlreadyReceived),
            PurchaseOrderStatus::Cancelled => Err(PurchasingError::OrderAlreadyCancelled),
        }
    }

    fn bump(&mut self) -> Result<(), PurchasingError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(PurchasingError::RevisionOverflow)?;
        Ok(())
    }
}

/// Datos de creación de una partida.
///
/// Los snapshots se derivan del artículo de inventario resuelto: la partida
/// nunca acepta un código, nombre o unidad distintos de los del artículo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPurchaseOrderLine {
    /// Identidad estable de la partida.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Orden a la que pertenece.
    pub purchase_order_id: EntityId,
    /// Artículo de inventario referenciado.
    pub inventory_item_id: EntityId,
    /// Número de línea asignado por el servidor; único en la orden.
    pub line_number: u32,
    /// Código del artículo congelado.
    pub item_code_snapshot: CatalogCode,
    /// Nombre del artículo congelado.
    pub item_name_snapshot: DisplayName,
    /// Unidad base del artículo congelada.
    pub unit_snapshot: InventoryBaseUnit,
    /// Cantidad pedida, estrictamente positiva.
    pub quantity: Quantity,
    /// Costo unitario pactado, opcional y no negativo.
    pub unit_cost: Option<UnitPrice>,
}

/// Partida de una orden de compra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseOrderLine {
    id: EntityId,
    organization_id: EntityId,
    purchase_order_id: EntityId,
    inventory_item_id: EntityId,
    line_number: u32,
    item_code_snapshot: CatalogCode,
    item_name_snapshot: DisplayName,
    unit_snapshot: InventoryBaseUnit,
    quantity: Quantity,
    unit_cost: Option<UnitPrice>,
}

impl PurchaseOrderLine {
    /// Crea una partida validando el número de línea y la cantidad.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::InvalidLineNumber`] o
    /// [`PurchasingError::InvalidQuantity`].
    pub fn create(new: NewPurchaseOrderLine) -> Result<Self, PurchasingError> {
        if new.line_number == 0 {
            return Err(PurchasingError::InvalidLineNumber);
        }
        if new.quantity.is_negative() || new.quantity.is_zero() {
            return Err(PurchasingError::InvalidQuantity);
        }
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            purchase_order_id: new.purchase_order_id,
            inventory_item_id: new.inventory_item_id,
            line_number: new.line_number,
            item_code_snapshot: new.item_code_snapshot,
            item_name_snapshot: new.item_name_snapshot,
            unit_snapshot: new.unit_snapshot,
            quantity: new.quantity,
            unit_cost: new.unit_cost,
        })
    }

    /// Rehidrata una partida persistida validando cantidad y línea.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::InvalidPersistedValue`].
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        purchase_order_id: EntityId,
        inventory_item_id: EntityId,
        line_number: u32,
        item_code_snapshot: CatalogCode,
        item_name_snapshot: DisplayName,
        unit_snapshot: InventoryBaseUnit,
        quantity: Quantity,
        unit_cost: Option<UnitPrice>,
    ) -> Result<Self, PurchasingError> {
        if line_number == 0 || quantity.is_negative() || quantity.is_zero() {
            return Err(PurchasingError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            purchase_order_id,
            inventory_item_id,
            line_number,
            item_code_snapshot,
            item_name_snapshot,
            unit_snapshot,
            quantity,
            unit_cost,
        })
    }

    /// Identidad de la partida.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Orden a la que pertenece.
    pub fn purchase_order_id(&self) -> &EntityId {
        &self.purchase_order_id
    }
    /// Artículo de inventario.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }
    /// Número de línea.
    pub fn line_number(&self) -> u32 {
        self.line_number
    }
    /// Código congelado del artículo.
    pub fn item_code_snapshot(&self) -> &CatalogCode {
        &self.item_code_snapshot
    }
    /// Nombre congelado del artículo.
    pub fn item_name_snapshot(&self) -> &DisplayName {
        &self.item_name_snapshot
    }
    /// Unidad congelada del artículo.
    pub fn unit_snapshot(&self) -> InventoryBaseUnit {
        self.unit_snapshot
    }
    /// Cantidad pedida.
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
    /// Costo unitario pactado.
    pub fn unit_cost(&self) -> Option<UnitPrice> {
        self.unit_cost
    }

    /// Verifica que la unidad del artículo no haya cambiado desde la captura.
    ///
    /// # Errors
    /// Devuelve [`PurchasingError::UnitChanged`] si difiere.
    pub fn ensure_unit_unchanged(&self, current: InventoryBaseUnit) -> Result<(), PurchasingError> {
        if self.unit_snapshot != current {
            return Err(PurchasingError::UnitChanged);
        }
        Ok(())
    }
}

/// Línea solicitada en una recepción.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptLineRequest {
    /// Partida de la orden.
    pub purchase_order_line_id: EntityId,
    /// Cantidad recibida, estrictamente positiva.
    pub quantity: Quantity,
}

/// Violación de cantidad detectada al recibir.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverReceiptViolation {
    /// Número de línea de la orden.
    pub line_number: u32,
    /// Cantidad pedida.
    pub ordered: Quantity,
    /// Cantidad ya recibida antes de esta recepción.
    pub already_received: Quantity,
    /// Cantidad intentada en esta recepción.
    pub attempted: Quantity,
}

/// Valida que las líneas de una recepción sean positivas y no se repitan.
///
/// # Errors
/// Devuelve [`PurchasingError::InvalidQuantity`] o
/// [`PurchasingError::DuplicateReceiptLine`].
pub fn validate_receipt_lines(requested: &[ReceiptLineRequest]) -> Result<(), PurchasingError> {
    let mut seen = std::collections::BTreeSet::new();
    for line in requested {
        if line.quantity.is_negative() || line.quantity.is_zero() {
            return Err(PurchasingError::InvalidQuantity);
        }
        if !seen.insert(&line.purchase_order_line_id) {
            return Err(PurchasingError::DuplicateReceiptLine);
        }
    }
    Ok(())
}

/// Calcula las líneas que excederían la cantidad pedida.
///
/// El orden de salida es el de `requested`, igual que el servicio Java.
pub fn over_receipt_violations(
    order_lines: &[PurchaseOrderLine],
    already_received: &[(EntityId, Quantity)],
    requested: &[ReceiptLineRequest],
) -> Vec<OverReceiptViolation> {
    let mut violations = Vec::new();
    for request in requested {
        let Some(line) = order_lines
            .iter()
            .find(|line| line.id == request.purchase_order_line_id)
        else {
            continue;
        };
        let previous = already_received
            .iter()
            .find(|(id, _)| id == &line.id)
            .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
        let total = match previous.checked_add(&request.quantity) {
            Ok(total) => total,
            Err(_) => line.quantity,
        };
        if total > line.quantity {
            violations.push(OverReceiptViolation {
                line_number: line.line_number,
                ordered: line.quantity,
                already_received: previous,
                attempted: request.quantity,
            });
        }
    }
    violations
}

/// Rechaza la recepción si alguna línea excede lo pedido.
///
/// # Errors
/// Devuelve [`PurchasingError::ReceiptExceedsOrdered`].
pub fn ensure_within_ordered(
    order_lines: &[PurchaseOrderLine],
    already_received: &[(EntityId, Quantity)],
    requested: &[ReceiptLineRequest],
) -> Result<(), PurchasingError> {
    if over_receipt_violations(order_lines, already_received, requested).is_empty() {
        Ok(())
    } else {
        Err(PurchasingError::ReceiptExceedsOrdered)
    }
}

/// Calcula el estado resultante tras aplicar una recepción.
///
/// Devuelve `RECEIVED` solo cuando todas las líneas alcanzan su cantidad pedida.
pub fn next_order_status(
    order_lines: &[PurchaseOrderLine],
    already_received: &[(EntityId, Quantity)],
    requested: &[ReceiptLineRequest],
) -> PurchaseOrderStatus {
    for line in order_lines {
        let previous = already_received
            .iter()
            .find(|(id, _)| id == &line.id)
            .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
        let this_receipt = requested
            .iter()
            .find(|request| request.purchase_order_line_id == line.id)
            .map_or_else(Quantity::zero, |request| request.quantity);
        let total = previous.checked_add(&this_receipt).unwrap_or(line.quantity);
        if total < line.quantity {
            return PurchaseOrderStatus::PartiallyReceived;
        }
    }
    PurchaseOrderStatus::Received
}

/// Campos modificados de una orden, con los nombres de la auditoría Java.
pub fn purchase_order_changed_fields(
    before: &PurchaseOrder,
    after: &PurchaseOrder,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.status != after.status {
        changed.push("status");
    }
    if before.supplier_id != after.supplier_id {
        changed.push("supplierId");
    }
    if before.supplier_code_snapshot != after.supplier_code_snapshot
        || before.supplier_name_snapshot != after.supplier_name_snapshot
    {
        changed.push("supplierSnapshot");
    }
    changed
}
