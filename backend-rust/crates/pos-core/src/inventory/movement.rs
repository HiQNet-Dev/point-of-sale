use super::*;
use crate::identity::UtcSeconds;
use crate::organization_branch::EntityId;

/// Motivo normalizado de un ajuste o apertura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovementReason(String);

impl MovementReason {
    /// Normaliza el motivo; exige contenido y respeta el límite.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::ReasonRequired`] si queda vacío y
    /// [`InventoryError::ReasonTooLong`] si supera el límite.
    pub fn new(value: &str) -> Result<Self, InventoryError> {
        let trimmed = value.trim_matches(|character: char| character <= '\u{20}');
        if trimmed.is_empty() {
            return Err(InventoryError::ReasonRequired);
        }
        if utf16_len(trimmed) > MAX_REASON_UNITS {
            return Err(InventoryError::ReasonTooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`] si no lo cumple.
    pub fn restore(value: &str) -> Result<Self, InventoryError> {
        match Self::new(value) {
            Ok(parsed) if parsed.as_str() == value => Ok(parsed),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tipo de movimiento del ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryMovementType {
    /// Apertura inicial de existencias (`>= 0`, una por sucursal y artículo).
    Opening,
    /// Ajuste manual (`<> 0`, motivo obligatorio).
    Adjustment,
    /// Consumo por venta (`< 0`, venta obligatoria).
    SaleConsumption,
    /// Recepción de compra (`> 0`, línea de recepción obligatoria).
    PurchaseReceipt,
}

impl InventoryMovementType {
    /// Valor persistido.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opening => "OPENING",
            Self::Adjustment => "ADJUSTMENT",
            Self::SaleConsumption => "SALE_CONSUMPTION",
            Self::PurchaseReceipt => "PURCHASE_RECEIPT",
        }
    }

    /// Convierte un valor persistido sin aceptar tipos desconocidos.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`].
    pub fn parse(value: &str) -> Result<Self, InventoryError> {
        match value {
            "OPENING" => Ok(Self::Opening),
            "ADJUSTMENT" => Ok(Self::Adjustment),
            "SALE_CONSUMPTION" => Ok(Self::SaleConsumption),
            "PURCHASE_RECEIPT" => Ok(Self::PurchaseReceipt),
            _ => Err(InventoryError::InvalidPersistedValue),
        }
    }
}

/// Hecho inmutable del ledger de existencias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryMovement {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    inventory_item_id: EntityId,
    movement_type: InventoryMovementType,
    quantity_delta: Quantity,
    resulting_quantity: Quantity,
    reason: Option<MovementReason>,
    actor_user_id: EntityId,
    occurred_at: UtcSeconds,
    sale_id: Option<EntityId>,
    purchase_receipt_line_id: Option<EntityId>,
}

/// Datos comunes de un movimiento.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMovement {
    /// Identidad del hecho.
    pub id: EntityId,
    /// Organización del scope.
    pub organization_id: EntityId,
    /// Sucursal del scope.
    pub branch_id: EntityId,
    /// Artículo afectado.
    pub inventory_item_id: EntityId,
    /// Actor autenticado; nunca proviene del cliente.
    pub actor_user_id: EntityId,
    /// Instante UTC del hecho.
    pub occurred_at: UtcSeconds,
}

impl InventoryMovement {
    /// Registra una apertura de existencias.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::OpeningMustBeNonNegative`].
    pub fn opening(
        new: NewMovement,
        quantity: Quantity,
        resulting_quantity: Quantity,
    ) -> Result<Self, InventoryError> {
        if quantity.is_negative() {
            return Err(InventoryError::OpeningMustBeNonNegative);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::Opening,
            quantity,
            resulting_quantity,
            None,
            None,
            None,
        ))
    }

    /// Registra un ajuste manual.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::AdjustmentMustNotBeZero`] si el delta es nulo.
    pub fn adjustment(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        reason: MovementReason,
    ) -> Result<Self, InventoryError> {
        if delta.is_zero() {
            return Err(InventoryError::AdjustmentMustNotBeZero);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::Adjustment,
            delta,
            resulting_quantity,
            Some(reason),
            None,
            None,
        ))
    }

    /// Registra el consumo de una venta.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::SaleConsumptionMustBeNegative`] si el delta no
    /// es negativo.
    pub fn sale_consumption(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        sale_id: EntityId,
    ) -> Result<Self, InventoryError> {
        if !delta.is_negative() {
            return Err(InventoryError::SaleConsumptionMustBeNegative);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::SaleConsumption,
            delta,
            resulting_quantity,
            None,
            Some(sale_id),
            None,
        ))
    }

    /// Registra la recepción de una línea de compra.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::PurchaseReceiptMustBePositive`] si el delta no
    /// es positivo.
    pub fn purchase_receipt(
        new: NewMovement,
        delta: Quantity,
        resulting_quantity: Quantity,
        receipt_line_id: EntityId,
    ) -> Result<Self, InventoryError> {
        if delta.is_negative() || delta.is_zero() {
            return Err(InventoryError::PurchaseReceiptMustBePositive);
        }
        Ok(Self::build(
            new,
            InventoryMovementType::PurchaseReceipt,
            delta,
            resulting_quantity,
            None,
            None,
            Some(receipt_line_id),
        ))
    }

    fn build(
        new: NewMovement,
        movement_type: InventoryMovementType,
        quantity_delta: Quantity,
        resulting_quantity: Quantity,
        reason: Option<MovementReason>,
        sale_id: Option<EntityId>,
        purchase_receipt_line_id: Option<EntityId>,
    ) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            branch_id: new.branch_id,
            inventory_item_id: new.inventory_item_id,
            movement_type,
            quantity_delta,
            resulting_quantity,
            reason,
            actor_user_id: new.actor_user_id,
            occurred_at: new.occurred_at,
            sale_id,
            purchase_receipt_line_id,
        }
    }

    /// Rehidrata un movimiento persistido validando la coherencia de su tipo.
    ///
    /// # Errors
    /// Devuelve [`InventoryError::InvalidPersistedValue`] si el delta, el motivo
    /// o las referencias no corresponden al tipo.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        inventory_item_id: EntityId,
        movement_type: InventoryMovementType,
        quantity_delta: Quantity,
        resulting_quantity: Quantity,
        reason: Option<MovementReason>,
        actor_user_id: EntityId,
        occurred_at: UtcSeconds,
        sale_id: Option<EntityId>,
        purchase_receipt_line_id: Option<EntityId>,
    ) -> Result<Self, InventoryError> {
        let coherent = match movement_type {
            InventoryMovementType::Opening => {
                !quantity_delta.is_negative()
                    && reason.is_none()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::Adjustment => {
                !quantity_delta.is_zero()
                    && reason.is_some()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::SaleConsumption => {
                quantity_delta.is_negative()
                    && reason.is_none()
                    && sale_id.is_some()
                    && purchase_receipt_line_id.is_none()
            }
            InventoryMovementType::PurchaseReceipt => {
                !quantity_delta.is_negative()
                    && !quantity_delta.is_zero()
                    && reason.is_none()
                    && sale_id.is_none()
                    && purchase_receipt_line_id.is_some()
            }
        };
        if !coherent {
            return Err(InventoryError::InvalidPersistedValue);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            inventory_item_id,
            movement_type,
            quantity_delta,
            resulting_quantity,
            reason,
            actor_user_id,
            occurred_at,
            sale_id,
            purchase_receipt_line_id,
        })
    }

    /// Identidad del hecho.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización del scope.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal del scope.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }
    /// Artículo afectado.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }
    /// Tipo del movimiento.
    pub fn movement_type(&self) -> InventoryMovementType {
        self.movement_type
    }
    /// Delta aplicado al saldo.
    pub fn quantity_delta(&self) -> Quantity {
        self.quantity_delta
    }
    /// Saldo resultante declarado.
    pub fn resulting_quantity(&self) -> Quantity {
        self.resulting_quantity
    }
    /// Motivo, obligatorio en ajustes.
    pub fn reason(&self) -> Option<&MovementReason> {
        self.reason.as_ref()
    }
    /// Actor autenticado.
    pub fn actor_user_id(&self) -> &EntityId {
        &self.actor_user_id
    }
    /// Instante UTC del hecho.
    pub fn occurred_at(&self) -> UtcSeconds {
        self.occurred_at
    }
    /// Venta de origen del consumo.
    pub fn sale_id(&self) -> Option<&EntityId> {
        self.sale_id.as_ref()
    }
    /// Línea de recepción de origen.
    pub fn purchase_receipt_line_id(&self) -> Option<&EntityId> {
        self.purchase_receipt_line_id.as_ref()
    }
}
