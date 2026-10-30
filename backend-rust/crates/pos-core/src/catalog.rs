//! Dominio puro del catálogo organizacional: categorías, productos y modificadores.
//!
//! Reproduce las reglas del backend Java (`catalog/domain/*`) sin JPA, sin Spring
//! y sin SQLx. El catálogo pertenece a la **organización**; la sucursal solo lo
//! consume en operación. Ninguna entidad se borra físicamente: se retira con
//! [`Status::Inactive`], y esa retirada nunca se propaga a otra fila.
//!
//! # Invariantes
//!
//! - `code` es un slug ASCII normalizado (`java_trim` + minúsculas) e inmutable.
//! - `sku` es el identificador de negocio, normalizado a mayúsculas e inmutable.
//! - `barcode` es opcional, se conserva como texto y distingue `None` de vacío.
//! - Los importes usan escala decimal exacta 4, nunca coma flotante.
//! - Los semánticos de selección de un grupo de modificadores se derivan de
//!   `min_selections`/`max_selections`; jamás se almacenan.
//! - La versión optimista arranca en `0` y solo avanza ante un cambio material.
//!
//! # Límite conocido
//!
//! El contrato físico `NUMERIC(19,4)` admite magnitudes que no caben en un
//! `INTEGER` de SQLite. Este módulo conserva el rango lógico en [`Decimal4`] y
//! expone [`Decimal4::as_minor_i64`] para que un adaptador decida cómo
//! persistirlo. La representación física definitiva del catálogo Local sigue
//! pendiente de aprobación.

use std::fmt;

use crate::organization_branch::{DisplayName, EntityId, Status, java_trim};

/// Longitud máxima en unidades UTF-16 de un código de catálogo.
pub const MAX_CATALOG_CODE_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 del SKU canónico.
pub const MAX_SKU_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 de un código de barras.
pub const MAX_BARCODE_UNITS: usize = 64;
/// Longitud máxima en unidades UTF-16 de una descripción.
pub const MAX_DESCRIPTION_UNITS: usize = 1000;
/// Escala decimal heredada del contrato `NUMERIC(19,4)`.
pub const PRICE_SCALE: u32 = 4;
/// Dígitos enteros máximos heredados del contrato `NUMERIC(19,4)`.
pub const MAX_PRICE_INTEGER_DIGITS: usize = 15;

const MINOR_FACTOR: i128 = 10_000;
const MAX_MINOR_MAGNITUDE: i128 = 10_000 * 1_000_000_000_000_000;

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Fallos del catálogo sin datos de entrada ni contenido sensible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogError {
    /// El código no es un slug ASCII válido o supera el límite.
    InvalidCode,
    /// El SKU no cumple `^[A-Z0-9._-]{1,64}$` tras normalizarse.
    InvalidSku,
    /// El código de barras supera el límite tras el recorte.
    InvalidBarcode,
    /// La descripción supera el límite tras el recorte.
    InvalidDescription,
    /// El nombre no satisface el value object común de nombre visible.
    InvalidName,
    /// El importe no es un decimal exacto admitido por `NUMERIC(19,4)`.
    InvalidPrice,
    /// Los límites de selección del grupo son incoherentes.
    InvalidSelectionRange,
    /// El orden de presentación es negativo.
    InvalidDisplayOrder,
    /// La versión esperada no corresponde al agregado actual.
    ConcurrentModification,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// La lista solicitada repite un grupo de modificadores.
    DuplicateModifierGroup,
    /// La categoría de destino está inactiva.
    CategoryInactive,
    /// La entidad referenciada pertenece a otra organización.
    OwnershipMismatch,
    /// Una fila persistida incumple las invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCode => "código de catálogo no válido",
            Self::InvalidSku => "SKU no válido",
            Self::InvalidBarcode => "código de barras no válido",
            Self::InvalidDescription => "descripción no válida",
            Self::InvalidName => "nombre no válido",
            Self::InvalidPrice => "importe no válido",
            Self::InvalidSelectionRange => "límites de selección no válidos",
            Self::InvalidDisplayOrder => "orden de presentación no válido",
            Self::ConcurrentModification => "modificación concurrente",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::DuplicateModifierGroup => "la lista repite un grupo de modificadores",
            Self::CategoryInactive => "la categoría está inactiva",
            Self::OwnershipMismatch => "la entidad pertenece a otra organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for CatalogError {}

/// Código de catálogo: slug ASCII minúsculo e inmutable.
///
/// Normaliza con `java_trim` y minúsculas ASCII, y exige
/// `^[a-z0-9]+(-[a-z0-9]+)*$` con hasta [`MAX_CATALOG_CODE_UNITS`] unidades
/// UTF-16. Una entrada Unicode se normaliza igual que en Java y termina
/// rechazada por la gramática, que solo admite ASCII.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CatalogCode(String);

impl CatalogCode {
    /// Normaliza y valida el código.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidCode`] si el resultado no es un slug.
    pub fn new(value: &str) -> Result<Self, CatalogError> {
        let normalized = java_trim(value).to_ascii_lowercase();
        let bytes = normalized.as_bytes();
        let valid = !bytes.is_empty()
            && utf16_len(&normalized) <= MAX_CATALOG_CODE_UNITS
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
            && !normalized.starts_with('-')
            && !normalized.ends_with('-')
            && !normalized.contains("--");
        if !valid {
            return Err(CatalogError::InvalidCode);
        }
        Ok(Self(normalized))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        let parsed = Self::new(value).map_err(|_| CatalogError::InvalidPersistedValue)?;
        if parsed.as_str() != value {
            return Err(CatalogError::InvalidPersistedValue);
        }
        Ok(parsed)
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// SKU canónico: identificador de negocio inmutable de un producto.
///
/// Normaliza con `java_trim` y mayúsculas Unicode, y exige
/// `^[A-Z0-9._-]{1,64}$` con hasta [`MAX_SKU_UNITS`] unidades UTF-16.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sku(String);

impl Sku {
    /// Normaliza y valida el SKU.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSku`] si el resultado no cumple la gramática.
    pub fn new(value: &str) -> Result<Self, CatalogError> {
        let normalized = java_trim(value).to_uppercase();
        let bytes = normalized.as_bytes();
        let valid = !bytes.is_empty()
            && utf16_len(&normalized) <= MAX_SKU_UNITS
            && bytes.iter().all(|byte| {
                byte.is_ascii_uppercase()
                    || byte.is_ascii_digit()
                    || matches!(*byte, b'.' | b'_' | b'-')
            });
        if !valid {
            return Err(CatalogError::InvalidSku);
        }
        Ok(Self(normalized))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        let parsed = Self::new(value).map_err(|_| CatalogError::InvalidPersistedValue)?;
        if parsed.as_str() != value {
            return Err(CatalogError::InvalidPersistedValue);
        }
        Ok(parsed)
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Código de barras opcional conservado como texto.
///
/// Un valor en blanco se normaliza a `None`; nunca se recorta por dentro ni se
/// rellena, para preservar ceros iniciales.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Barcode(String);

impl Barcode {
    /// Normaliza el código de barras, devolviendo `None` si queda vacío.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidBarcode`] si supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, CatalogError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        if utf16_len(trimmed) > MAX_BARCODE_UNITS {
            return Err(CatalogError::InvalidBarcode);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(CatalogError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Descripción opcional de producto.
///
/// Un valor en blanco se normaliza a `None` para que "sin descripción" tenga una
/// única representación, igual que el agregado Java.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Description(String);

impl Description {
    /// Normaliza la descripción, devolviendo `None` si queda vacía.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidDescription`] si supera el límite.
    pub fn new(value: &str) -> Result<Option<Self>, CatalogError> {
        let trimmed = java_trim(value);
        if trimmed.is_empty() {
            return Ok(None);
        }
        if utf16_len(trimmed) > MAX_DESCRIPTION_UNITS {
            return Err(CatalogError::InvalidDescription);
        }
        Ok(Some(Self(trimmed.to_owned())))
    }

    /// Lectura de una fila persistida que ya cumple el contrato.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPersistedValue`] si la fila no lo cumple.
    pub fn restore(value: &str) -> Result<Self, CatalogError> {
        match Self::new(value) {
            Ok(Some(parsed)) if parsed.as_str() == value => Ok(parsed),
            _ => Err(CatalogError::InvalidPersistedValue),
        }
    }

    /// Representación persistible.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Decimal exacto con escala fija 4, sin coma flotante.
///
/// Acepta un signo opcional, uno o más dígitos enteros y, si aparece el punto,
/// al menos un decimal. Rechaza exponentes, separadores, espacios y más de
/// cuatro decimales, incluidos los ceros finales: `1.50000` tiene escala 5 y el
/// agregado Java también lo rechaza.
///
/// Los dígitos enteros significativos no pueden superar
/// [`MAX_PRICE_INTEGER_DIGITS`]. La comparación se hace sobre la magnitud, de
/// modo que los ceros iniciales no alteran el resultado.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Decimal4(i128);

impl Decimal4 {
    /// Interpreta un decimal canónico ya expresado con hasta cuatro decimales.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la sintaxis o el rango no
    /// corresponden al contrato.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        let (negative, unsigned) = if let Some(rest) = input.strip_prefix('-') {
            (true, rest)
        } else if let Some(rest) = input.strip_prefix('+') {
            (false, rest)
        } else {
            (false, input)
        };
        let (integer, fraction) = match unsigned.split_once('.') {
            Some((integer, fraction)) => {
                if fraction.is_empty() {
                    return Err(CatalogError::InvalidPrice);
                }
                (integer, fraction)
            }
            None => (unsigned, ""),
        };
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(CatalogError::InvalidPrice);
        }
        if fraction.len() > PRICE_SCALE as usize
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(CatalogError::InvalidPrice);
        }
        let significant = integer.trim_start_matches('0');
        if significant.len() > MAX_PRICE_INTEGER_DIGITS {
            return Err(CatalogError::InvalidPrice);
        }
        let mut magnitude: i128 = 0;
        for byte in integer.bytes() {
            magnitude = magnitude * 10 + i128::from(byte - b'0');
        }
        magnitude *= MINOR_FACTOR;
        let mut scaled_fraction: i128 = 0;
        for byte in fraction.bytes() {
            scaled_fraction = scaled_fraction * 10 + i128::from(byte - b'0');
        }
        for _ in fraction.len()..PRICE_SCALE as usize {
            scaled_fraction *= 10;
        }
        magnitude += scaled_fraction;
        if magnitude >= MAX_MINOR_MAGNITUDE {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(if negative { -magnitude } else { magnitude }))
    }

    /// Construye desde unidades menores ya expresadas con escala 4.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la magnitud excede el rango
    /// lógico de `NUMERIC(19,4)`.
    pub fn from_minor(minor: i128) -> Result<Self, CatalogError> {
        if minor.unsigned_abs() >= MAX_MINOR_MAGNITUDE.unsigned_abs() {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(minor))
    }

    /// Unidades menores con escala 4.
    pub fn as_minor(&self) -> i128 {
        self.0
    }

    /// Unidades menores si caben en el `INTEGER` de SQLite.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] cuando el valor lógico no cabe en
    /// un entero firmado de 64 bits; la representación física del catálogo
    /// Local sigue pendiente de aprobación.
    pub fn as_minor_i64(&self) -> Result<i64, CatalogError> {
        i64::try_from(self.0).map_err(|_| CatalogError::InvalidPrice)
    }

    /// Cero exacto con escala 4.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Indica si el valor es estrictamente negativo.
    pub fn is_negative(&self) -> bool {
        self.0 < 0
    }

    /// Indica si el valor es exactamente cero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Decimal4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let integer = magnitude / MINOR_FACTOR.unsigned_abs();
        let fraction = magnitude % MINOR_FACTOR.unsigned_abs();
        if self.0 < 0 {
            formatter.write_str("-")?;
        }
        write!(formatter, "{integer}.{fraction:04}")
    }
}

/// Precio base de producto: decimal exacto no negativo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitPrice(Decimal4);

impl UnitPrice {
    /// Acepta únicamente valores no negativos.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si el valor es negativo.
    pub fn new(value: Decimal4) -> Result<Self, CatalogError> {
        if value.is_negative() {
            return Err(CatalogError::InvalidPrice);
        }
        Ok(Self(value))
    }

    /// Interpreta el texto decimal y aplica la regla de no negatividad.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] en cualquier incumplimiento.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        Self::new(Decimal4::parse(input)?)
    }

    /// Importe cero.
    pub fn zero() -> Self {
        Self(Decimal4::zero())
    }

    /// Valor decimal subyacente.
    pub fn value(&self) -> Decimal4 {
        self.0
    }
}

impl fmt::Display for UnitPrice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Delta de precio firmado de una opción de modificador.
///
/// Admite descuentos (negativo), sin cambio (cero) y recargos (positivo).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceDelta(Decimal4);

impl PriceDelta {
    /// Envuelve un decimal ya validado.
    pub fn new(value: Decimal4) -> Self {
        Self(value)
    }

    /// Interpreta el texto decimal con signo.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidPrice`] si la sintaxis o el rango fallan.
    pub fn parse(input: &str) -> Result<Self, CatalogError> {
        Ok(Self(Decimal4::parse(input)?))
    }

    /// Valor decimal subyacente.
    pub fn value(&self) -> Decimal4 {
        self.0
    }
}

impl fmt::Display for PriceDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

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

/// Datos de creación de una categoría.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCategory {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
}

/// Categoría de producto propiedad de una organización.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    status: Status,
    revision: u64,
}

impl Category {
    /// Crea una categoría activa con revisión `0`.
    pub fn create(new: NewCategory) -> Self {
        Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una categoría persistida.
    ///
    /// Uso exclusivo de adaptadores: el código y el nombre ya llegan validados.
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            code,
            name,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria; el código solo es único dentro de ella.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Código inmutable.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la categoría admite nuevas asignaciones de producto.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`] si la versión esperada
    /// no coincide y [`CatalogError::RevisionOverflow`] si la revisión no cabe.
    pub fn patch(&mut self, patch: CategoryPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
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
}

/// Patch de categoría: el código y la organización nunca se modifican.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

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
            product.category_id = Some(category.id);
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

/// Datos de creación de un grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModifierGroup {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Organización propietaria.
    pub organization_id: EntityId,
    /// Código ya validado.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Mínimo de opciones exigidas (`>= 0`).
    pub min_selections: u32,
    /// Máximo de opciones admitidas (`>= 1`).
    pub max_selections: u32,
}

/// Grupo de modificadores reutilizable propiedad de una organización.
///
/// Los semánticos `required`/`single`/`multi` se derivan de los límites y nunca
/// se almacenan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroup {
    id: EntityId,
    organization_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    min_selections: u32,
    max_selections: u32,
    status: Status,
    revision: u64,
}

impl ModifierGroup {
    /// Crea un grupo activo con revisión `0`.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSelectionRange`] si `max_selections` es
    /// cero o `min_selections` lo supera.
    pub fn create(new: NewModifierGroup) -> Result<Self, CatalogError> {
        validate_selections(new.min_selections, new.max_selections)?;
        Ok(Self {
            id: new.id,
            organization_id: new.organization_id,
            code: new.code,
            name: new.name,
            min_selections: new.min_selections,
            max_selections: new.max_selections,
            status: Status::Active,
            revision: 0,
        })
    }

    /// Rehidrata un grupo persistido validando sus límites.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::InvalidSelectionRange`] si la fila es incoherente.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        min_selections: u32,
        max_selections: u32,
        status: Status,
        revision: u64,
    ) -> Result<Self, CatalogError> {
        validate_selections(min_selections, max_selections)?;
        Ok(Self {
            id,
            organization_id,
            code,
            name,
            min_selections,
            max_selections,
            status,
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

    /// Código inmutable.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Mínimo de opciones exigidas.
    pub fn min_selections(&self) -> u32 {
        self.min_selections
    }

    /// Máximo de opciones admitidas.
    pub fn max_selections(&self) -> u32 {
        self.max_selections
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si el grupo es seleccionable en operación.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Derivado: al menos una opción es obligatoria.
    pub fn is_required(&self) -> bool {
        self.min_selections > 0
    }

    /// Derivado: exactamente una opción.
    pub fn is_single_choice(&self) -> bool {
        self.max_selections == 1
    }

    /// Derivado: más de una opción.
    pub fn is_multiple_choice(&self) -> bool {
        self.max_selections > 1
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`],
    /// [`CatalogError::InvalidSelectionRange`] o [`CatalogError::RevisionOverflow`].
    pub fn patch(&mut self, patch: ModifierGroupPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if patch.min_selections.is_some() || patch.max_selections.is_some() {
            let min = patch.min_selections.unwrap_or(next.min_selections);
            let max = patch.max_selections.unwrap_or(next.max_selections);
            validate_selections(min, max)?;
            changed |= next.min_selections != min || next.max_selections != max;
            next.min_selections = min;
            next.max_selections = max;
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
}

/// Patch de grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierGroupPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Mínimo nuevo; si se omite se conserva el vigente.
    pub min_selections: Option<u32>,
    /// Máximo nuevo; si se omite se conserva el vigente.
    pub max_selections: Option<u32>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Datos de creación de una opción de modificador.
///
/// La organización se deriva del grupo: una opción nunca puede pertenecer a una
/// organización distinta de la de su grupo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewModifierOption {
    /// Identidad estable asignada por la aplicación.
    pub id: EntityId,
    /// Grupo propietario ya resuelto y de la organización solicitada.
    pub group: ModifierGroup,
    /// Código ya validado; único dentro del grupo.
    pub code: CatalogCode,
    /// Nombre ya validado.
    pub name: DisplayName,
    /// Delta firmado sobre el precio base.
    pub price_delta: PriceDelta,
    /// Orden de presentación dentro del grupo.
    pub display_order: u32,
}

/// Opción de un grupo de modificadores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierOption {
    id: EntityId,
    organization_id: EntityId,
    group_id: EntityId,
    code: CatalogCode,
    name: DisplayName,
    price_delta: PriceDelta,
    display_order: u32,
    status: Status,
    revision: u64,
}

impl ModifierOption {
    /// Crea una opción activa con revisión `0`.
    ///
    /// La actividad del grupo no se exige, igual que en el agregado Java: solo
    /// se comprueba la pertenencia a la organización, que aquí se deriva del
    /// propio grupo.
    pub fn create(new: NewModifierOption) -> Self {
        Self {
            id: new.id,
            organization_id: new.group.organization_id().clone(),
            group_id: new.group.id().clone(),
            code: new.code,
            name: new.name,
            price_delta: new.price_delta,
            display_order: new.display_order,
            status: Status::Active,
            revision: 0,
        }
    }

    /// Rehidrata una opción persistida.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        organization_id: EntityId,
        group_id: EntityId,
        code: CatalogCode,
        name: DisplayName,
        price_delta: PriceDelta,
        display_order: u32,
        status: Status,
        revision: u64,
    ) -> Self {
        Self {
            id,
            organization_id,
            group_id,
            code,
            name,
            price_delta,
            display_order,
            status,
            revision,
        }
    }

    /// Identidad estable.
    pub fn id(&self) -> &EntityId {
        &self.id
    }

    /// Organización propietaria, derivada del grupo.
    pub fn organization_id(&self) -> &EntityId {
        &self.organization_id
    }

    /// Grupo propietario.
    pub fn group_id(&self) -> &EntityId {
        &self.group_id
    }

    /// Código inmutable y único dentro del grupo.
    pub fn code(&self) -> &CatalogCode {
        &self.code
    }

    /// Nombre visible.
    pub fn name(&self) -> &DisplayName {
        &self.name
    }

    /// Delta firmado sobre el precio base.
    pub fn price_delta(&self) -> PriceDelta {
        self.price_delta
    }

    /// Orden de presentación.
    pub fn display_order(&self) -> u32 {
        self.display_order
    }

    /// Estado vigente.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Revisión optimista.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Indica si la opción es seleccionable en operación.
    pub fn is_active(&self) -> bool {
        self.status == Status::Active
    }

    /// Aplica un patch atómico; devuelve `true` solo si algo cambió.
    ///
    /// # Errors
    /// Devuelve [`CatalogError::ConcurrentModification`] o
    /// [`CatalogError::RevisionOverflow`].
    pub fn patch(&mut self, patch: ModifierOptionPatch) -> Result<bool, CatalogError> {
        if patch.expected_version != self.revision {
            return Err(CatalogError::ConcurrentModification);
        }
        let mut next = self.clone();
        let mut changed = false;
        if let Some(value) = patch.name {
            changed |= next.name != value;
            next.name = value;
        }
        if let Some(value) = patch.price_delta {
            changed |= next.price_delta != value;
            next.price_delta = value;
        }
        if let Some(value) = patch.display_order {
            changed |= next.display_order != value;
            next.display_order = value;
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
}

/// Patch de opción de modificador.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModifierOptionPatch {
    /// Versión previamente leída.
    pub expected_version: u64,
    /// Nombre nuevo, ya validado.
    pub name: Option<DisplayName>,
    /// Delta nuevo.
    pub price_delta: Option<PriceDelta>,
    /// Orden nuevo.
    pub display_order: Option<u32>,
    /// Estado nuevo.
    pub status: Option<Status>,
}

/// Resultado puro del reemplazo de la lista de grupos de un producto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModifierReplacement {
    /// La lista solicitada coincide con la vigente: sin mutación ni auditoría.
    Unchanged,
    /// Configuración nueva en el orden solicitado.
    Replace(Vec<EntityId>),
}

/// Calcula el reemplazo ordenado de la configuración de modificadores.
///
/// La posición en la lista es el orden de presentación. Una lista idéntica es
/// una operación nula; una lista con identificadores repetidos se rechaza.
///
/// # Errors
/// Devuelve [`CatalogError::DuplicateModifierGroup`] si hay identificadores
/// repetidos.
pub fn plan_modifier_replacement(
    current: &[EntityId],
    requested: &[EntityId],
) -> Result<ModifierReplacement, CatalogError> {
    let mut seen = std::collections::BTreeSet::new();
    for group in requested {
        if !seen.insert(group) {
            return Err(CatalogError::DuplicateModifierGroup);
        }
    }
    if current == requested {
        return Ok(ModifierReplacement::Unchanged);
    }
    Ok(ModifierReplacement::Replace(requested.to_vec()))
}

/// Campos modificados de una categoría, con los nombres de la auditoría Java.
pub fn category_changed_fields(before: &Category, after: &Category) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
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

/// Campos modificados de un grupo de modificadores.
pub fn modifier_group_changed_fields(
    before: &ModifierGroup,
    after: &ModifierGroup,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.min_selections != after.min_selections
        || before.max_selections != after.max_selections
    {
        changed.push("selections");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}

/// Campos modificados de una opción de modificador.
pub fn modifier_option_changed_fields(
    before: &ModifierOption,
    after: &ModifierOption,
) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if before.name != after.name {
        changed.push("name");
    }
    if before.price_delta != after.price_delta {
        changed.push("priceDelta");
    }
    if before.display_order != after.display_order {
        changed.push("displayOrder");
    }
    if before.status != after.status {
        changed.push("status");
    }
    changed
}

fn validate_selections(min: u32, max: u32) -> Result<(), CatalogError> {
    if max == 0 || min > max {
        return Err(CatalogError::InvalidSelectionRange);
    }
    Ok(())
}

/// Convierte un orden de presentación de transporte a `u32`.
///
/// # Errors
/// Devuelve [`CatalogError::InvalidDisplayOrder`] si el valor es negativo.
pub fn display_order(value: i64) -> Result<u32, CatalogError> {
    u32::try_from(value).map_err(|_| CatalogError::InvalidDisplayOrder)
}
