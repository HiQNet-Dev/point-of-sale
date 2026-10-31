use super::*;
use crate::organization_branch::{DisplayName, EntityId, Status};

/// Datos de creación de un producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewProduct {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// SKU canónico.
    pub sku: Sku,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Precio base no negativo.
    pub unit_price: UnitPrice,
    /// Código de barras opcional ya normalizado.
    pub barcode: Option<Barcode>,
    /// Descripción opcional ya normalizada.
    pub description: Option<Description>,
    /// Categoría resuelta, activa y de la misma organización.
    pub category: Option<Category>,
}

/// Producto con precio base propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    id: EntityId,
    organization_id: EntityId,
    category_id: Option<EntityId>,
    sku: Sku,
    barcode: Option<Barcode>,
    name: DisplayName,
    description: Option<Description>,
    unit_price: UnitPrice,
    status: Status,
    revision: u64,
}

impl Product {
    /// Crea un producto activo con revisión `0`.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::OwnershipMismatch`] si la categoría es de otra
    /// organización y [`CatalogError::CategoryInactive`] si no está activa.
    pub fn create(new: NewProduct) -> Result<Self, CatalogError> {
        let mut product = Self {
            id: new.id,
            organization_id: new.organization_id,
            category_id: None,
            sku: new.sku,
            barcode: new.barcode,
            name: new.name,
            description: new.description,
            unit_price: new.unit_price,
            status: Status::Active,
            revision: 0,
        };
        if let Some(category) = new.category {
            product.validate_category(&category)?;
            product.category_id = Some(category.id().clone());
        }
        Ok(product)
    }

    /// Rehidrata un producto persistido.
    ///
    /// Uso exclusivo de adaptadores: todos los valores llegan ya validados.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        category_id: Option<EntityId>,
        sku: Sku,
        barcode: Option<Barcode>,
        name: DisplayName,
        description: Option<Description>,
        unit_price: UnitPrice,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            category_id,
            sku,
            barcode,
            name,
            description,
            unit_price,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Categoría asignada, si existe.
    pub fn category_id(&self) -> Option<&EntityId> {
        self.category_id.as_ref()
    }

    /// SKU canónico e inmutable.
    pub fn sku(&self) -> &Sku {
        &self.sku
    }

    /// Código de barras opcional.
    pub fn barcode(&self) -> Option<&Barcode> {
        self.barcode.as_ref()
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Descripción opcional.
    pub fn description(&self) -> Option<&Description> {
        self.description.as_ref()
    }

    /// Precio base de la organización.
    pub fn unit_price(&self) -> UnitPrice {
        self.unit_price
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el producto es vendible en operación.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// El SKU y la organización no son modificables. La categoría solo se
    /// asigna si está activa y pertenece a la misma organización.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`],
    /// [`CatalogError::OwnershipMismatch`], [`CatalogError::CategoryInactive`] o
    /// [`CatalogError::RevisionOverflow`].
    pub fn patch(&mut self, patch: ProductPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        match patch.description {
            PatchField::Absent => {}
            PatchField::Clear => {
                changed |= next.description.is_some();
                next.description = None;
            }
            PatchField::Set(value) => {
                changed |= next.description.as_ref() != Some(&value);
                next.description = Some(value);
            }
        }
        match patch.barcode {
            PatchField::Absent => {}
            PatchField::Clear => {
                changed |= next.barcode.is_some();
                next.barcode = None;
            }
            PatchField::Set(value) => {
                changed |= next.barcode.as_ref() != Some(&value);
                next.barcode = Some(value);
            }
        }
        if let Some(value) = patch.unit_price {
            changed |= next.unit_price != value;
            next.unit_price = value;
        }
        match patch.category {
            PatchField::Absent => {}
            PatchField::Clear => {
                changed |= next.category_id.is_some();
                next.category_id = None;
            }
            PatchField::Set(category) => {
                next.validate_category(&category)?;
                changed |= next.category_id.as_ref() != Some(category.id());
                next.category_id = Some(category.id().clone());
            }
        }
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(CatalogError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }

    /// Marca la configuración de modificadores como modificada.
    ///
    /// El producto es la frontera de concurrencia de su configuración: no existe
    /// una versión separada. Se invoca solo después de un reemplazo real.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`] si la versión no
    /// coincide y [`CatalogError::RevisionOverflow`] si la revisión no cabe.
    pub fn mark_configuration_changed(
        &mut self,
        expected_version: u64,
    ) -> Result<(), CatalogError> {
        if expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(CatalogError::RevisionOverflow)?;
        Ok(())
    }

    fn validate_category(&self, category: &Category) -> Result<(), CatalogError> {
        if category.organization_id() != &self.organization_id {
            return Err(CatalogError::OwnershipMismatch);
        }
        if !category.is_active() {
            return Err(CatalogError::CategoryInactive);
        }
        Ok(())
    }
}

/// Patch de producto con distinción explícita de ausencia y limpieza.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Descripción: ausente, limpiada o nueva.
    pub description: PatchField<Description>,
    /// Código de barras: ausente, limpiado o nuevo.
    pub barcode: PatchField<Barcode>,
    /// Precio base nuevo.
    pub unit_price: Option<UnitPrice>,
    /// Categoría resuelta: ausente, limpiada o nueva.
    pub category: PatchField<Category>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Campos modificados de un producto, con los nombres de la auditoría Java.
pub fn product_changed_fields(before: &Product, after: &Product) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.description != after.description {
        changed.push("description");
    }
    if before.barcode != after.barcode {
        changed.push("barcode");
    }
    if before.unit_price != after.unit_price {
        changed.push("unitPrice");
    }
    if before.category_id != after.category_id {
        changed.push("categoryId");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
