//! Dominio puro de proveedores de compras.
//!
//! Reproduce `purchasing/domain/Supplier` y `SupplierService` sin JPA, Spring,
//! SQLx ni transporte. El proveedor es dato maestro de la organización: nunca se
//! elimina, se retira con [`Status::Inactive`] para conservar el historial de
//! órdenes y recepciones.
//!
//! # Invariantes
//!
//! - `code` es un slug ASCII minúsculo, único por organización e inmutable.
//! - `name` es obligatorio; el resto de campos de contacto es opcional y el
//!   blanco se normaliza a `None`.
//! - El email se recorta, se pliega a minúsculas y debe cumplir
//!   `^[^@\s]+@[^@\s]+\.[^@\s]+$`.
//! - La revisión optimista arranca en `0` y solo avanza con un cambio material.
//! - [`Supplier::snapshot`] produce los datos que `pos-core::purchasing` congela
//!   en la orden; no existe una segunda representación del proveedor.

use std::fmt;

use crate::{
    catalog::{CatalogCode, PatchField},
    organization_branch::{DisplayName, EntityId, Status, java_trim},
};

/// Longitud máxima del nombre comercial (unidades UTF-16).
pub const MAX_SUPPLIER_NAME_UNITS: usize = 120;
/// Longitud máxima de la razón social.
pub const MAX_LEGAL_NAME_UNITS: usize = 160;
/// Longitud máxima del identificador fiscal.
pub const MAX_TAX_ID_UNITS: usize = 64;
/// Longitud máxima del contacto principal.
pub const MAX_CONTACT_NAME_UNITS: usize = 120;
/// Longitud máxima del email de contacto.
pub const MAX_EMAIL_UNITS: usize = 254;
/// Longitud máxima del teléfono.
pub const MAX_PHONE_UNITS: usize = 40;
/// Longitud máxima de la dirección.
pub const MAX_ADDRESS_UNITS: usize = 500;

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Fallos de proveedor sin contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupplierError {
    /// Un campo de texto opcional supera su límite.
    TextTooLong,
    /// El email no cumple el formato heredado.
    InvalidEmail,
    /// La versión esperada no corresponde al proveedor.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La entidad pertenece a otra organización.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for SupplierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TextTooLong => "campo de texto fuera de límite",
            Self::InvalidEmail => "email no válido",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "el proveedor pertenece a otra organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for SupplierError {}

/// Texto opcional recortado con un límite fijo en unidades UTF-16.
///
/// El blanco se normaliza a `None`, igual que `Supplier.normalizeOptional` de
/// Java. El límite forma parte del tipo, de modo que no puede confundirse la
/// razón social con el teléfono.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedText<const MAX: usize>(String);

impl<const MAX: usize> BoundedText<MAX> {
    /// Normaliza el texto; devuelve `None` si queda vacío.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::TextTooLong`] si supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, SupplierError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        if utf16_len(trimmed) > MAX {
            return Err(SupplierError::TextTooLong);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::InvalidPersistedValue`] si no lo cumple.
    pub fn restore(value: &str) -> Result<Self, SupplierError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(SupplierError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Email de contacto normalizado.
///
/// Recorta, pliega a minúsculas con semántica Unicode y exige la forma
/// `^[^@\s]+@[^@\s]+\.[^@\s]+$`. El espacio en blanco considerado es el de Java
/// (`[\t\n\u{0B}\u{0C}\r ]`), no todo el espacio Unicode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplierEmail(String);

impl SupplierEmail {
    /// Normaliza el email; devuelve `None` si queda vacío.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::InvalidEmail`] si el formato no es válido o
    /// supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, SupplierError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        let normalized = trimmed.to_lowercase();
        if utf16_len(&normalized) > MAX_EMAIL_UNITS || !well_formed_email(&normalized) {
            return Err(SupplierError::InvalidEmail);
        }
        Ok(Some(Self(normalized)))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::InvalidPersistedValue`] si no lo cumple.
    pub fn restore(value: &str) -> Result<Self, SupplierError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(SupplierError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_java_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn well_formed_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.chars().any(|c| c == '@' || is_java_space(c)) {
        return false;
    }
    if domain.is_empty() || domain.chars().any(|c| c == '@' || is_java_space(c)) {
        return false;
    }
    match domain.rfind('.') {
        Some(index) => index > 0 && index + 1 < domain.len(),
        None => false,
    }
}

/// Datos de creación de un proveedor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewSupplier {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado; único por organización e inmutable.
    pub code: CatalogCode,
    /// Nombre comercial ya validado.
    pub name: DisplayName,
    /// Razón social opcional.
    pub legal_name: Option<BoundedText<MAX_LEGAL_NAME_UNITS>>,
    /// Identificador fiscal opcional.
    pub tax_id: Option<BoundedText<MAX_TAX_ID_UNITS>>,
    /// Contacto principal opcional.
    pub contact_name: Option<BoundedText<MAX_CONTACT_NAME_UNITS>>,
    /// Email de contacto opcional.
    pub email: Option<SupplierEmail>,
    /// Teléfono opcional.
    pub phone: Option<BoundedText<MAX_PHONE_UNITS>>,
    /// Dirección opcional.
    pub address: Option<BoundedText<MAX_ADDRESS_UNITS>>,
}

/// Datos del proveedor congelados en una orden de compra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplierSnapshot {
    /// Código comercial vigente al crear la orden.
    pub code: CatalogCode,
    /// Nombre comercial vigente al crear la orden.
    pub name: DisplayName,
}

/// Proveedor propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Supplier {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    legal_name: Option<BoundedText<MAX_LEGAL_NAME_UNITS>>,
    tax_id: Option<BoundedText<MAX_TAX_ID_UNITS>>,
    contact_name: Option<BoundedText<MAX_CONTACT_NAME_UNITS>>,
    email: Option<SupplierEmail>,
    phone: Option<BoundedText<MAX_PHONE_UNITS>>,
    address: Option<BoundedText<MAX_ADDRESS_UNITS>>,
    status: Status,
    revision: u64,
}

impl Supplier {
    /// Crea un proveedor activo con revisión `0`.
    pub fn create(new: NewSupplier) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            legal_name: new.legal_name,
            tax_id: new.tax_id,
            contact_name: new.contact_name,
            email: new.email,
            phone: new.phone,
            address: new.address,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata un proveedor persistido.
    ///
    /// Uso exclusivo de adaptadores: los valores ya llegan validados.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        legal_name: Option<BoundedText<MAX_LEGAL_NAME_UNITS>>,
        tax_id: Option<BoundedText<MAX_TAX_ID_UNITS>>,
        contact_name: Option<BoundedText<MAX_CONTACT_NAME_UNITS>>,
        email: Option<SupplierEmail>,
        phone: Option<BoundedText<MAX_PHONE_UNITS>>,
        address: Option<BoundedText<MAX_ADDRESS_UNITS>>,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            legal_name,
            tax_id,
            contact_name,
            email,
            phone,
            address,
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
    /// Código inmutable y único por organización.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }
    /// Nombre comercial.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }
    /// Razón social opcional.
    pub fn legal_name(&self) -> Option<&str> {
        self.legal_name.as_ref().map(BoundedText::as_str)
    }
    /// Identificador fiscal opcional.
    pub fn tax_id(&self) -> Option<&str> {
        self.tax_id.as_ref().map(BoundedText::as_str)
    }
    /// Contacto principal opcional.
    pub fn contact_name(&self) -> Option<&str> {
        self.contact_name.as_ref().map(BoundedText::as_str)
    }
    /// Email de contacto normalizado.
    pub fn email(&self) -> Option<&str> {
        self.email.as_ref().map(SupplierEmail::as_str)
    }
    /// Teléfono opcional.
    pub fn phone(&self) -> Option<&str> {
        self.phone.as_ref().map(BoundedText::as_str)
    }
    /// Dirección opcional.
    pub fn address(&self) -> Option<&str> {
        self.address.as_ref().map(BoundedText::as_str)
    }
    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }
    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el proveedor admite nuevas órdenes.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Comprueba la pertenencia a una organización.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::OwnershipMismatch`].
    pub fn ensure_organization(&self, organization_id: &EntityId) -> Result<(), SupplierError> {
        if &self.organization_id != organization_id {
            return Err(SupplierError::OwnershipMismatch);
        }
        Ok(())
    }

    /// Datos que una orden de compra congela al crearse.
    pub fn snapshot(&self) -> SupplierSnapshot {
        SupplierSnapshot {
            code: self.code.clone(),
            name: self.name.clone(),
        }
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// El código y la organización no son modificables.
    ///
    /// # Errors
    /// Devuelve [`SupplierError::ConcurrentModification`] o
    /// [`SupplierError::RevisionOverflow`].
    pub fn patch(&mut self, patch: SupplierPatch) -> Result<bool, SupplierError> {
        if patch.expected_version != self.revision {
            return Err(SupplierError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        changed |= apply_optional(&mut next.legal_name, patch.legal_name);
        changed |= apply_optional(&mut next.tax_id, patch.tax_id);
        changed |= apply_optional(&mut next.contact_name, patch.contact_name);
        changed |= apply_optional(&mut next.email, patch.email);
        changed |= apply_optional(&mut next.phone, patch.phone);
        changed |= apply_optional(&mut next.address, patch.address);
        if let Some(value) = patch.status {
            changed |= next.status != value;
            next.status = value;
        }
        if changed {
            next.revision = next
                .revision
                .checked_add(1)
                .ok_or(SupplierError::RevisionOverflow)?;
        }
        *self = next;
        Ok(changed)
    }
}

fn apply_optional<T: Eq + Clone>(field: &mut Option<T>, patch: PatchField<T>) -> bool {
    match patch {
        PatchField::Absent => false,
        PatchField::Clear => {
            let changed = field.is_some();
            *field = None;
            changed
        }
        PatchField::Set(value) => {
            let changed = field.as_ref() != Some(&value);
            *field = Some(value);
            changed
        }
    }
}

/// Patch de proveedor con distinción explícita de ausencia y limpieza.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplierPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre comercial nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Razón social.
    pub legal_name: PatchField<BoundedText<MAX_LEGAL_NAME_UNITS>>,
    /// Identificador fiscal.
    pub tax_id: PatchField<BoundedText<MAX_TAX_ID_UNITS>>,
    /// Contacto principal.
    pub contact_name: PatchField<BoundedText<MAX_CONTACT_NAME_UNITS>>,
    /// Email de contacto.
    pub email: PatchField<SupplierEmail>,
    /// Teléfono.
    pub phone: PatchField<BoundedText<MAX_PHONE_UNITS>>,
    /// Dirección.
    pub address: PatchField<BoundedText<MAX_ADDRESS_UNITS>>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Campos modificados de un proveedor, con los nombres de la auditoría Java.
pub fn supplier_changed_fields(before: &Supplier, after: &Supplier) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.legal_name != after.legal_name {
        changed.push("legalName");
    }
    if before.tax_id != after.tax_id {
        changed.push("taxId");
    }
    if before.contact_name != after.contact_name {
        changed.push("contactName");
    }
    if before.email != after.email {
        changed.push("email");
    }
    if before.phone != after.phone {
        changed.push("phone");
    }
    if before.address != after.address {
        changed.push("address");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}
