package com.hiqnet.pos.purchasing.api;

import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * Verifica que PostgreSQL es la autoridad final de integridad del modelo de
 * órdenes de compra: FKs organization-scoped, unicidad, formato y coherencia de
 * estado se validan en la base, no solo en el dominio.
 */
class PurchaseOrderDbConstraintsTest extends AbstractRbacIntegrationTest {

    private static final String INSERT_ORDER = "INSERT INTO purchase_orders "
            + "(id, organization_id, branch_id, supplier_id, code, status, currency, supplier_code_snapshot, "
            + "supplier_name_snapshot, created_by_user_id, version, created_at, updated_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_LINE = "INSERT INTO purchase_order_lines "
            + "(id, organization_id, purchase_order_id, inventory_item_id, line_number, item_code_snapshot, "
            + "item_name_snapshot, unit_snapshot, quantity, unit_cost, created_at, updated_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private void insertOrder(UUID orgId, UUID branchId, UUID supplierId, UUID userId, String code, String status) {
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(
                INSERT_ORDER, UUID.randomUUID(), orgId, branchId, supplierId, code, status, "MXN", "proveedor",
                "Proveedor", userId, 0, now, now);
    }

    @Test
    void databaseEnforcesOrderScopeFormatStatusAndSnapshotConstraints() {
        RbacFixture fixture = fixture();
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);

        RbacFixture other = fixture();
        Branch foreignBranch = branchService.create(other.orgId(), "Ajena", "sucursal-ajena", null);
        Supplier foreignSupplier = supplierService.create(
                other.orgId(), "proveedor-ajeno", "Proveedor Ajeno", null, null, null, null, null, null);

        // Sucursal de otra organización.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), foreignBranch.getId(), supplier.getId(), fixture.userId(), "po-1", "DRAFT"));
        // Proveedor de otra organización.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), branch.getId(), foreignSupplier.getId(), fixture.userId(), "po-1", "DRAFT"));
        // Código con formato inválido.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "PO-1", "DRAFT"));
        // Estado desconocido.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "po-1", "SHIPPED"));
        // Coherencia de estado: ORDERED exige actor y fecha de emisión.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "po-ordered", "ORDERED"));

        insertOrder(fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "po-1", "DRAFT");
        // Código duplicado dentro de la organización.
        assertThrows(DataIntegrityViolationException.class, () -> insertOrder(
                fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "po-1", "DRAFT"));
    }

    @Test
    void databaseEnforcesLineQuantityCostUniquenessAndScope() {
        RbacFixture fixture = fixture();
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item = inventoryItemService.create(
                fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        InventoryItem secondItem = inventoryItemService.create(
                fixture.orgId(), "azucar", "Azúcar", null, InventoryBaseUnit.GRAM, true);

        RbacFixture other = fixture();
        InventoryItem foreignItem = inventoryItemService.create(
                other.orgId(), "ajeno", "Artículo ajeno", null, InventoryBaseUnit.GRAM, true);

        insertOrder(fixture.orgId(), branch.getId(), supplier.getId(), fixture.userId(), "po-1", "DRAFT");
        UUID orderId = jdbcTemplate.queryForObject(
                "select id from purchase_orders where organization_id = ? and code = 'po-1'",
                UUID.class,
                fixture.orgId());

        Timestamp now = Timestamp.from(Instant.now());
        insertLine(orderId, fixture.orgId(), item.getId(), 1, "10.000000", "1.0000", "GRAM", now);

        // Cantidad no positiva.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), secondItem.getId(), 2, "0.000000", null, "GRAM", now));
        // Costo negativo.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), secondItem.getId(), 2, "1.000000", "-1.0000", "GRAM", now));
        // Unidad desconocida.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), secondItem.getId(), 2, "1.000000", null, "KILOGRAM", now));
        // Número de línea inválido.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), secondItem.getId(), 0, "1.000000", null, "GRAM", now));
        // Artículo de otra organización.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), foreignItem.getId(), 2, "1.000000", null, "GRAM", now));
        // Artículo duplicado en la misma orden.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), item.getId(), 2, "1.000000", null, "GRAM", now));
        // Número de línea duplicado.
        assertThrows(DataIntegrityViolationException.class, () -> insertLine(
                orderId, fixture.orgId(), secondItem.getId(), 1, "1.000000", null, "GRAM", now));
    }

    private void insertLine(
            UUID orderId,
            UUID orgId,
            UUID itemId,
            int lineNumber,
            String quantity,
            String unitCost,
            String unit,
            Timestamp now) {
        jdbcTemplate.update(
                INSERT_LINE,
                UUID.randomUUID(),
                orgId,
                orderId,
                itemId,
                lineNumber,
                "item",
                "Item",
                unit,
                new java.math.BigDecimal(quantity),
                unitCost == null ? null : new java.math.BigDecimal(unitCost),
                now,
                now);
    }
}
