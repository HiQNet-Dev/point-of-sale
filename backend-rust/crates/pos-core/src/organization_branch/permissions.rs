/// Permisos que estos casos de uso requieren.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    /// Provisioning inicial, solo por bootstrap explícito.
    BootstrapOrganization,
    /// Lectura de organización.
    OrganizationRead,
    /// Actualización de organización.
    OrganizationUpdate,
    /// Alta de sucursal.
    BranchCreate,
    /// Lectura de sucursales.
    BranchRead,
    /// Actualización de sucursal.
    BranchUpdate,
    /// Alta de categoría de catálogo.
    CategoryCreate,
    /// Lectura de categorías.
    CategoryRead,
    /// Actualización de categoría.
    CategoryUpdate,
    /// Alta de producto.
    ProductCreate,
    /// Lectura de productos.
    ProductRead,
    /// Actualización de producto y de su precio base.
    ProductUpdate,
    /// Descubrimiento del catálogo operativo desde una sucursal.
    PosCatalogRead,
    /// Lectura de cajas de una sucursal.
    CashRegisterRead,
    /// Alta de cajas de una sucursal.
    CashRegisterCreate,
    /// Actualización de nombre o estado de una caja.
    CashRegisterUpdate,
    /// Lectura de turnos de una caja.
    CashShiftRead,
    /// Apertura de un turno en una caja activa.
    CashShiftOpen,
    /// Cierre de un turno abierto, incluido el camino de recuperación autorizado.
    CashShiftClose,
    /// Lectura de ventas de un turno de caja.
    SaleRead,
    /// Apertura de una venta operativa.
    SaleCreate,
    /// Alta, modificación y retirada lógica de partidas de una venta OPEN.
    SaleUpdate,
    /// Cancelación operativa de una venta OPEN.
    SaleCancel,
    /// Checkout integral de una venta con pagos e inventario atómicos.
    SaleCheckout,
    /// Lectura de artículos de inventario.
    InventoryItemRead,
    /// Alta y mantenimiento de artículos de inventario.
    InventoryItemWrite,
    /// Lectura de saldos de stock y del ledger de una sucursal.
    InventoryStockRead,
    /// Aperturas y ajustes de stock.
    InventoryStockAdjust,
    /// Lectura de recetas de producto y efectos de modificador.
    InventoryRecipeRead,
    /// Mantenimiento de recetas y efectos de inventario de modificadores.
    InventoryRecipeWrite,
}

impl Permission {
    /// Código canónico equivalente en el catálogo de permisos Java (V5–V25).
    ///
    /// Es la única traducción autorizada entre el enum de aplicación y el
    /// lenguaje que persiste el RBAC Local. [`Self::BootstrapOrganization`]
    /// devuelve `None`: el bootstrap no se concede con un permiso de rol, sino
    /// con la capacidad explícita de provisioning.
    pub const fn as_code(self) -> Option<&'static str> {
        match self {
            Self::BootstrapOrganization => None,
            Self::OrganizationRead => Some("organization.read"),
            Self::OrganizationUpdate => Some("organization.update"),
            Self::BranchCreate => Some("branch.create"),
            Self::BranchRead => Some("branch.read"),
            Self::BranchUpdate => Some("branch.update"),
            Self::CategoryCreate => Some("category.create"),
            Self::CategoryRead => Some("category.read"),
            Self::CategoryUpdate => Some("category.update"),
            Self::ProductCreate => Some("product.create"),
            Self::ProductRead => Some("product.read"),
            Self::ProductUpdate => Some("product.update"),
            Self::PosCatalogRead => Some("pos.catalog.read"),
            Self::CashRegisterRead => Some("cash-register.read"),
            Self::CashRegisterCreate => Some("cash-register.create"),
            Self::CashRegisterUpdate => Some("cash-register.update"),
            Self::CashShiftRead => Some("cash-shift.read"),
            Self::CashShiftOpen => Some("cash-shift.open"),
            Self::CashShiftClose => Some("cash-shift.close"),
            Self::SaleRead => Some("sale.read"),
            Self::SaleCreate => Some("sale.create"),
            Self::SaleUpdate => Some("sale.update"),
            Self::SaleCancel => Some("sale.cancel"),
            Self::SaleCheckout => Some("sale.checkout"),
            Self::InventoryItemRead => Some("inventory.item.read"),
            Self::InventoryItemWrite => Some("inventory.item.write"),
            Self::InventoryStockRead => Some("inventory.stock.read"),
            Self::InventoryStockAdjust => Some("inventory.stock.adjust"),
            Self::InventoryRecipeRead => Some("inventory.recipe.read"),
            Self::InventoryRecipeWrite => Some("inventory.recipe.write"),
        }
    }
}
