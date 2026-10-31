//! Recepciones de compra: hechos inmutables que incrementan stock.
//!
//! Una recepción agrupa las cantidades efectivamente entregadas por el
//! proveedor. No almacena valoración económica: el costo efectivo de recepción
//! sigue pendiente en ADR-0021 y no puede inferirse del costo pactado.
//!
//! # Invariantes
//!
//! - Cada recepción pertenece a una orden y a la sucursal de esa orden.
//! - La cantidad de cada línea es estrictamente positiva y exacta (escala 6).
//! - Una partida de la orden no puede recibirse dos veces en la misma recepción.
//! - La clave de idempotencia es única por organización: repetir el envío no
//!   crea una segunda recepción.
//! - La recepción no se modifica ni se elimina; los errores se corrigen con un
//!   hecho nuevo autorizado, nunca reescribiendo historia.

use crate::{
    inventory::Quantity,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator},
};

/// Datos de creación de una recepción.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPurchaseReceipt {
    /// Identidad asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Sucursal de la orden.
    pub branch_id: EntityId,
    /// Orden recibida.
    pub purchase_order_id: EntityId,
    /// Clave de idempotencia declarada por el cliente.
    pub idempotency_key: String,
    /// Actor autenticado que registra la recepción.
    pub actor_user_id: EntityId,
}

/// Recepción confirmada.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseReceipt {
    id: EntityId,
    organization_id: EntityId,
    branch_id: EntityId,
    purchase_order_id: EntityId,
    idempotency_key: String,
    actor_user_id: EntityId,
    received_at_seconds: i64,
}

impl PurchaseReceipt {
    /// Crea una recepción validando la clave de idempotencia.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::InvalidPurchaseInput`] si la clave está
    /// vacía, supera 120 caracteres o contiene caracteres de control.
    pub fn create(
        new: NewPurchaseReceipt,
        received_at_seconds: i64,
    ) -> Result<Self, ApplicationError> {
        if !valid_idempotency_key(&new.idempotency_key) || received_at_seconds < 0 {
            return Err(ApplicationError::InvalidPurchaseInput);
        }
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            branch_id: new.branch_id,
            purchase_order_id: new.purchase_order_id,
            idempotency_key: new.idempotency_key,
            actor_user_id: new.actor_user_id,
            received_at_seconds,
        })
    }

    /// Rehidrata una recepción persistida.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Storage`] si la fila es incoherente.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        branch_id: EntityId,
        purchase_order_id: EntityId,
        idempotency_key: String,
        actor_user_id: EntityId,
        received_at_seconds: i64,
    ) -> Result<Self, ApplicationError> {
        if !valid_idempotency_key(&idempotency_key) || received_at_seconds < 0 {
            return Err(ApplicationError::Storage);
        }
        Ok(Self {
            id,
            organization_id,
            branch_id,
            purchase_order_id,
            idempotency_key,
            actor_user_id,
            received_at_seconds,
        })
    }

    /// Identidad de la recepción.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Sucursal de la orden.
    pub fn branch_id(&self) -> &EntityId {
        &self.branch_id
    }
    /// Orden recibida.
    pub fn purchase_order_id(&self) -> &EntityId {
        &self.purchase_order_id
    }
    /// Clave de idempotencia.
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
    /// Actor que registró la recepción.
    pub fn actor_user_id(&self) -> &EntityId {
        &self.actor_user_id
    }
    /// Instante de recepción en segundos Unix.
    pub fn received_at_seconds(&self) -> i64 {
        self.received_at_seconds
    }
}

/// Línea de una recepción.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseReceiptLine {
    id: EntityId,
    organization_id: EntityId,
    purchase_receipt_id: EntityId,
    purchase_order_line_id: EntityId,
    inventory_item_id: EntityId,
    quantity: Quantity,
}

impl PurchaseReceiptLine {
    /// Crea una línea de recepción con cantidad estrictamente positiva.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::InvalidPurchaseInput`] si la cantidad no es
    /// positiva.
    pub fn create(
        id: EntityId,
        organization_id: EntityId,
        purchase_receipt_id: EntityId,
        purchase_order_line_id: EntityId,
        inventory_item_id: EntityId,
        quantity: Quantity,
    ) -> Result<Self, ApplicationError> {
        if quantity.is_zero() || quantity.is_negative() {
            return Err(ApplicationError::InvalidPurchaseInput);
        }
        Ok(Self {
            id,
            organization_id,
            purchase_receipt_id,
            purchase_order_line_id,
            inventory_item_id,
            quantity,
        })
    }

    /// Rehidrata una línea persistida.
    ///
    /// # Errors
    /// Devuelve [`ApplicationError::Storage`] si la cantidad no es positiva.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        purchase_receipt_id: EntityId,
        purchase_order_line_id: EntityId,
        inventory_item_id: EntityId,
        quantity: Quantity,
    ) -> Result<Self, ApplicationError> {
        if quantity.is_zero() || quantity.is_negative() {
            return Err(ApplicationError::Storage);
        }
        Ok(Self {
            id,
            organization_id,
            purchase_receipt_id,
            purchase_order_line_id,
            inventory_item_id,
            quantity,
        })
    }

    /// Identidad de la línea.
    pub fn id(&self) -> &EntityId {
        &self.id
    }
    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }
    /// Recepción a la que pertenece.
    pub fn purchase_receipt_id(&self) -> &EntityId {
        &self.purchase_receipt_id
    }
    /// Partida de la orden recibida.
    pub fn purchase_order_line_id(&self) -> &EntityId {
        &self.purchase_order_line_id
    }
    /// Artículo de inventario recibido.
    pub fn inventory_item_id(&self) -> &EntityId {
        &self.inventory_item_id
    }
    /// Cantidad recibida.
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
}

/// Genera identidades de líneas de recepción a partir del generador de la
/// aplicación; existe para no repetir el bucle en cada caso de uso.
pub fn allocate_receipt_line_ids<I: IdentityGenerator>(
    identities: &mut I,
    count: usize,
) -> Result<Vec<EntityId>, ApplicationError> {
    (0..count).map(|_| identities.next_id()).collect()
}

fn valid_idempotency_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

#[cfg(test)]
mod tests {
    // Los fixtures son constantes válidas; estos panics no pertenecen al código de producto.
    #![allow(clippy::panic)]

    use super::*;

    fn id(value: &str) -> EntityId {
        EntityId::parse(value).unwrap_or_else(|error| panic!("uuid: {error}"))
    }

    #[test]
    fn idempotency_key_and_positive_quantity_are_enforced() {
        let receipt = PurchaseReceipt::create(
            NewPurchaseReceipt {
                id: id("00000000-0000-0000-0000-0000000009a1"),
                organization_id: id("00000000-0000-0000-0000-000000000001"),
                branch_id: id("00000000-0000-0000-0000-000000000002"),
                purchase_order_id: id("00000000-0000-0000-0000-0000000009a2"),
                idempotency_key: "receipt-1".to_owned(),
                actor_user_id: id("00000000-0000-0000-0000-0000000009a3"),
            },
            1_793_350_800,
        );
        assert!(receipt.is_ok());

        for key in ["", " ", "line\nbreak"] {
            assert_eq!(
                PurchaseReceipt::create(
                    NewPurchaseReceipt {
                        id: id("00000000-0000-0000-0000-0000000009a1"),
                        organization_id: id("00000000-0000-0000-0000-000000000001"),
                        branch_id: id("00000000-0000-0000-0000-000000000002"),
                        purchase_order_id: id("00000000-0000-0000-0000-0000000009a2"),
                        idempotency_key: key.to_owned(),
                        actor_user_id: id("00000000-0000-0000-0000-0000000009a3"),
                    },
                    1_793_350_800,
                ),
                Err(ApplicationError::InvalidPurchaseInput)
            );
        }

        let zero = Quantity::parse("0.000000");
        assert!(zero.is_ok());
        let Ok(zero) = zero else { return };
        assert_eq!(
            PurchaseReceiptLine::create(
                id("00000000-0000-0000-0000-0000000009a4"),
                id("00000000-0000-0000-0000-000000000001"),
                id("00000000-0000-0000-0000-0000000009a1"),
                id("00000000-0000-0000-0000-0000000009a5"),
                id("00000000-0000-0000-0000-0000000009a6"),
                zero,
            ),
            Err(ApplicationError::InvalidPurchaseInput)
        );
    }
}
