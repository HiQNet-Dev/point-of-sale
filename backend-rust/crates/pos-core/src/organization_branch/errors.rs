use std::fmt;

/// Errores puros del dominio; no representan estados HTTP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainError {
    /// El UUID no tiene formato canónico minúsculo.
    InvalidUuid,
    /// Un nombre es vacío tras el recorte Java o supera su límite UTF-16.
    InvalidName,
    /// Un código de negocio no es un slug ASCII válido.
    InvalidCode,
    /// La zona no representa un identificador compatible conocido.
    InvalidTimezone,
    /// El código no pertenece al catálogo `Currency` del JDK de referencia.
    InvalidCurrency,
    /// El tag BCP-47 no cumple el perfil compatible implementado.
    InvalidLocale,
    /// La revisión recibida ya no corresponde al agregado.
    StaleRevision,
    /// La siguiente revisión no cabe en el tipo persistible.
    RevisionOverflow,
    /// El owner inmutable de una sucursal no coincide con el solicitado.
    OwnershipMismatch,
    /// Una fila persistida incumple invariantes de este dominio.
    InvalidPersistedValue,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUuid => "UUID no canónico",
            Self::InvalidName => "nombre no válido",
            Self::InvalidCode => "código no válido",
            Self::InvalidTimezone => "zona horaria no válida",
            Self::InvalidCurrency => "moneda ISO 4217 no válida",
            Self::InvalidLocale => "locale BCP-47 no válido",
            Self::StaleRevision => "revisión obsoleta",
            Self::RevisionOverflow => "la revisión excede el rango permitido",
            Self::OwnershipMismatch => "la sucursal no pertenece a la organización",
            Self::InvalidPersistedValue => "fila persistida no válida",
        })
    }
}

impl std::error::Error for DomainError {}

/// Errores de la capa de aplicación, sin códigos HTTP ni detalles de infraestructura.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    /// La identidad no posee el permiso contextual.
    Forbidden,
    /// Organización inexistente.
    OrganizationNotFound,
    /// Sucursal inexistente en el scope solicitado.
    BranchNotFound,
    /// Código global de organización duplicado.
    OrganizationCodeAlreadyExists,
    /// Código de sucursal duplicado dentro del owner.
    BranchCodeAlreadyExists,
    /// Categoría inexistente en la organización del principal.
    CategoryNotFound,
    /// Producto inexistente en la organización del principal.
    ProductNotFound,
    /// Código de categoría duplicado dentro de la organización.
    CategoryCodeAlreadyExists,
    /// SKU duplicado dentro de la organización.
    ProductSkuAlreadyExists,
    /// Código de barras duplicado dentro de la organización.
    ProductBarcodeAlreadyExists,
    /// La categoría destino está inactiva.
    CategoryInactive,
    /// Caja inexistente en el scope solicitado.
    CashRegisterNotFound,
    /// Turno inexistente en el scope solicitado.
    CashShiftNotFound,
    /// Código de caja duplicado dentro de la sucursal.
    CashRegisterCodeAlreadyExists,
    /// La caja está inactiva y no admite aperturas.
    CashRegisterInactive,
    /// La sucursal está inactiva y no admite aperturas.
    BranchInactive,
    /// La caja ya tiene un turno abierto.
    CashRegisterAlreadyHasOpenShift,
    /// El turno ya fue cerrado y no es idempotente.
    CashShiftAlreadyClosed,
    /// El turno no puede cerrarse mientras haya ventas abiertas.
    CashShiftHasOpenSales,
    /// Entrada de caja o turno que no cumple el contrato.
    InvalidCashInput,
    /// Venta inexistente en el scope operativo solicitado.
    SaleNotFound,
    /// Partida inexistente dentro de la venta solicitada.
    SaleItemNotFound,
    /// La venta no está en estado OPEN.
    SaleNotOpen,
    /// La venta ya fue cancelada.
    SaleAlreadyCancelled,
    /// La venta ya fue completada.
    SaleAlreadyCompleted,
    /// La partida ya fue retirada lógicamente.
    SaleItemAlreadyRemoved,
    /// El producto no está activo para cotizar una nueva partida.
    ProductInactive,
    /// Entrada comercial que no cumple el contrato de ventas.
    InvalidSaleInput,
    /// Artículo de inventario inexistente en la organización del principal.
    InventoryItemNotFound,
    /// Código de artículo duplicado dentro de la organización.
    InventoryItemCodeAlreadyExists,
    /// El artículo de inventario no está activo para la operación.
    InventoryItemInactive,
    /// El artículo ya tiene apertura registrada.
    InventoryOpeningAlreadyExists,
    /// El stock disponible no cubre el consumo bajo política estricta.
    InsufficientInventory,
    /// Entrada de inventario o receta que no cumple el contrato.
    InvalidInventoryInput,
    /// Entrada de compras que no cumple el contrato.
    InvalidPurchaseInput,
    /// Proveedor inexistente en la organización del principal.
    SupplierNotFound,
    /// Código de proveedor duplicado dentro de la organización.
    SupplierCodeAlreadyExists,
    /// El proveedor está inactivo y no admite nuevas órdenes.
    SupplierInactive,
    /// Orden de compra inexistente en el scope solicitado.
    PurchaseOrderNotFound,
    /// Código de orden duplicado dentro de la organización.
    PurchaseOrderCodeAlreadyExists,
    /// La orden no está en borrador y no admite la operación.
    PurchaseOrderNotDraft,
    /// La orden ya fue emitida y no admite una segunda emisión.
    PurchaseOrderAlreadyIssued,
    /// La orden ya fue cancelada.
    PurchaseOrderAlreadyCancelled,
    /// La orden no admite recepciones en su estado actual.
    PurchaseOrderNotReceivable,
    /// Partida de compra inexistente en la orden.
    PurchaseOrderLineNotFound,
    /// La recepción solicitada duplica cantidades ya recibidas.
    OverReceipt,
    /// La clave de idempotencia ya fue utilizada por otra recepción.
    DuplicateReceipt,
    /// Recepción de compra inexistente en el scope solicitado.
    PurchaseReceiptNotFound,
    /// La entidad referenciada pertenece a otra organización.
    OwnershipMismatch,
    /// Una entrada de catálogo incumple el contrato de dominio.
    InvalidCatalogInput,
    /// CAS perdido después de haber validado el patch.
    ConcurrentModification,
    /// Fallo de dominio conservando su tipado.
    Domain(DomainError),
    /// Fallo del adaptador sin filtrar detalles sensibles.
    Storage,
    /// Fallo obligatorio de auditoría.
    Audit,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forbidden => formatter.write_str("operación no autorizada"),
            Self::OrganizationNotFound => formatter.write_str("organización inexistente"),
            Self::BranchNotFound => formatter.write_str("sucursal inexistente"),
            Self::OrganizationCodeAlreadyExists => {
                formatter.write_str("código de organización duplicado")
            }
            Self::BranchCodeAlreadyExists => formatter.write_str("código de sucursal duplicado"),
            Self::CategoryNotFound => formatter.write_str("categoría inexistente"),
            Self::ProductNotFound => formatter.write_str("producto inexistente"),
            Self::CategoryCodeAlreadyExists => formatter.write_str("código de categoría duplicado"),
            Self::ProductSkuAlreadyExists => formatter.write_str("SKU duplicado"),
            Self::ProductBarcodeAlreadyExists => formatter.write_str("código de barras duplicado"),
            Self::CategoryInactive => formatter.write_str("la categoría está inactiva"),
            Self::CashRegisterNotFound => formatter.write_str("caja inexistente"),
            Self::CashShiftNotFound => formatter.write_str("turno inexistente"),
            Self::CashRegisterCodeAlreadyExists => formatter.write_str("código de caja duplicado"),
            Self::CashRegisterInactive => formatter.write_str("la caja está inactiva"),
            Self::BranchInactive => formatter.write_str("la sucursal está inactiva"),
            Self::CashRegisterAlreadyHasOpenShift => {
                formatter.write_str("la caja ya tiene un turno abierto")
            }
            Self::CashShiftAlreadyClosed => formatter.write_str("el turno ya está cerrado"),
            Self::CashShiftHasOpenSales => formatter.write_str("el turno tiene ventas abiertas"),
            Self::InvalidCashInput => formatter.write_str("entrada de caja o turno no válida"),
            Self::SaleNotFound => formatter.write_str("venta inexistente"),
            Self::SaleItemNotFound => formatter.write_str("partida de venta inexistente"),
            Self::SaleNotOpen => formatter.write_str("la venta no está abierta"),
            Self::SaleAlreadyCancelled => formatter.write_str("la venta ya está cancelada"),
            Self::SaleAlreadyCompleted => formatter.write_str("la venta ya está completada"),
            Self::SaleItemAlreadyRemoved => formatter.write_str("la partida ya fue retirada"),
            Self::ProductInactive => formatter.write_str("el producto está inactivo"),
            Self::InvalidSaleInput => formatter.write_str("entrada de venta no válida"),
            Self::InventoryItemNotFound => {
                formatter.write_str("artículo de inventario inexistente")
            }
            Self::InventoryItemCodeAlreadyExists => {
                formatter.write_str("código de artículo de inventario duplicado")
            }
            Self::InventoryItemInactive => {
                formatter.write_str("el artículo de inventario está inactivo")
            }
            Self::InventoryOpeningAlreadyExists => {
                formatter.write_str("el artículo ya tiene apertura registrada")
            }
            Self::InsufficientInventory => formatter.write_str("existencias insuficientes"),
            Self::InvalidInventoryInput => formatter.write_str("entrada de inventario no válida"),
            Self::InvalidPurchaseInput => formatter.write_str("entrada de compras no válida"),
            Self::SupplierNotFound => formatter.write_str("proveedor inexistente"),
            Self::SupplierCodeAlreadyExists => formatter.write_str("código de proveedor duplicado"),
            Self::SupplierInactive => formatter.write_str("el proveedor está inactivo"),
            Self::PurchaseOrderNotFound => formatter.write_str("orden de compra inexistente"),
            Self::PurchaseOrderCodeAlreadyExists => {
                formatter.write_str("código de orden de compra duplicado")
            }
            Self::PurchaseOrderNotDraft => formatter.write_str("la orden no está en borrador"),
            Self::PurchaseOrderAlreadyIssued => formatter.write_str("la orden ya fue emitida"),
            Self::PurchaseOrderAlreadyCancelled => formatter.write_str("la orden ya fue cancelada"),
            Self::PurchaseOrderNotReceivable => {
                formatter.write_str("la orden no admite recepciones")
            }
            Self::PurchaseOrderLineNotFound => formatter.write_str("partida de compra inexistente"),
            Self::OverReceipt => formatter.write_str("cantidad recibida superior a la pedida"),
            Self::DuplicateReceipt => formatter.write_str("recepción repetida para la misma clave"),
            Self::PurchaseReceiptNotFound => formatter.write_str("recepción inexistente"),
            Self::OwnershipMismatch => {
                formatter.write_str("la entidad pertenece a otra organización")
            }
            Self::InvalidCatalogInput => formatter.write_str("entrada de catálogo no válida"),
            Self::ConcurrentModification => formatter.write_str("modificación concurrente"),
            Self::Domain(error) => error.fmt(formatter),
            Self::Storage => formatter.write_str("operación de almacenamiento fallida"),
            Self::Audit => formatter.write_str("auditoría obligatoria fallida"),
        }
    }
}

impl std::error::Error for ApplicationError {}

impl From<DomainError> for ApplicationError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
