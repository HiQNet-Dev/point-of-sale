package com.hiqnet.pos.purchasing.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Fixture compartida de las suites de recepción de mercancía: organización,
 * sucursal, proveedor y dos insumos sobre la cadena de seguridad real, más
 * ayudantes para crear órdenes emitidas, publicar recepciones y verificar el
 * estado final directamente en PostgreSQL (nunca a través de la API de lectura).
 */
abstract class AbstractPurchaseReceiptTest extends AbstractAuditIntegrationTest {

    protected static final String BASE = "/api/v1/organizations/";

    @Autowired protected BranchService branchService;

    @Autowired protected SupplierService supplierService;

    @Autowired protected InventoryItemService inventoryItemService;

    /** Ámbito de una organización con una sucursal, un proveedor y dos insumos. */
    protected record Scope(
            RbacFixture fixture, String token, UUID branchId, UUID supplierId, UUID itemA, UUID itemB) {}

    /** Orden emitida con sus partidas y la versión vigente tras la emisión. */
    protected record Order(String id, List<UUID> lineIds, long version) {}

    protected Scope scope() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem itemA = inventoryItemService.create(
                fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        InventoryItem itemB = inventoryItemService.create(
                fixture.orgId(), "azucar", "Azucar", null, InventoryBaseUnit.GRAM, true);
        return new Scope(fixture, token, branch.getId(), supplier.getId(), itemA.getId(), itemB.getId());
    }

    protected String ordersUrl(Scope scope) {
        return BASE + scope.fixture().orgId() + "/branches/" + scope.branchId() + "/purchase-orders";
    }

    protected String receiptsUrl(Scope scope, String orderId) {
        return ordersUrl(scope) + "/" + orderId + "/receipts";
    }

    protected String receiptStatusUrl(Scope scope, String orderId) {
        return ordersUrl(scope) + "/" + orderId + "/receipt-status";
    }

    /**
     * Crea y emite una orden con una partida por cantidad, usando los insumos A y
     * B en orden. La cantidad de argumentos es el número de partidas.
     */
    protected Order issuedOrder(Scope scope, String code, String... quantities) throws Exception {
        String orderId = draftOrder(scope, code);
        UUID[] items = {scope.itemA(), scope.itemB()};
        List<UUID> lineIds = new ArrayList<>();
        long version = 0;
        for (int index = 0; index < quantities.length; index++) {
            MvcResult line = mockMvc.perform(post(ordersUrl(scope) + "/" + orderId + "/lines")
                            .header("Authorization", bearer(scope.token()))
                            .contentType(MediaType.APPLICATION_JSON)
                            .content("{\"expectedVersion\":" + version + ",\"inventoryItemId\":\"" + items[index]
                                    + "\",\"quantity\":\"" + quantities[index] + "\"}"))
                    .andExpect(status().isCreated())
                    .andReturn();
            lineIds.add(UUID.fromString(objectMapper
                    .readTree(line.getResponse().getContentAsString())
                    .get("lines")
                    .get(index)
                    .get("id")
                    .asText()));
            version++;
        }
        mockMvc.perform(post(ordersUrl(scope) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + version + "}"))
                .andExpect(status().isOk());
        return new Order(orderId, lineIds, version + 1);
    }

    /** Crea una orden en borrador sin partidas (aún no emitida). */
    protected String draftOrder(Scope scope, String code) throws Exception {
        MvcResult created = mockMvc.perform(post(ordersUrl(scope))
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + scope.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(created.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
    }

    protected MvcResult postReceipt(
            Scope scope, String orderId, String key, long expectedVersion, UUID lineId, String quantity)
            throws Exception {
        return postReceipt(scope, orderId, key, expectedVersion, List.of(lineId), List.of(quantity));
    }

    /** Publica una recepción con las líneas tal como se pasan (permite duplicados a propósito). */
    protected MvcResult postReceipt(
            Scope scope, String orderId, String key, long expectedVersion, List<UUID> lineIds, List<String> quantities)
            throws Exception {
        StringBuilder lines = new StringBuilder();
        for (int index = 0; index < lineIds.size(); index++) {
            if (index > 0) {
                lines.append(',');
            }
            lines.append("{\"purchaseOrderLineId\":\"")
                    .append(lineIds.get(index))
                    .append("\",\"quantity\":\"")
                    .append(quantities.get(index))
                    .append("\"}");
        }
        return mockMvc.perform(post(receiptsUrl(scope, orderId))
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + ",\"idempotencyKey\":\"" + key
                                + "\",\"lines\":[" + lines + "]}"))
                .andReturn();
    }

    protected String errorCode(MvcResult result) throws Exception {
        return objectMapper.readTree(result.getResponse().getContentAsString()).get("code").asText();
    }

    protected String receiptId(MvcResult result) throws Exception {
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("purchaseReceipt")
                .get("id")
                .asText();
    }

    protected String receiptStatus(MvcResult result) throws Exception {
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("status")
                .asText();
    }

    /** Siembra un balance directo (acepta negativos según la política vigente). */
    protected void seedBalance(Scope scope, UUID itemId, String quantity) {
        jdbcTemplate.update(
                "insert into branch_inventory "
                        + "(id,organization_id,branch_id,inventory_item_id,quantity_on_hand,version,created_at,updated_at) "
                        + "values (?,?,?,?,?,0,now(),now())",
                UUID.randomUUID(),
                scope.fixture().orgId(),
                scope.branchId(),
                itemId,
                new BigDecimal(quantity));
    }

    protected BigDecimal balance(Scope scope, UUID itemId) {
        List<BigDecimal> rows = jdbcTemplate.queryForList(
                "select quantity_on_hand from branch_inventory where branch_id = ? and inventory_item_id = ?",
                BigDecimal.class,
                scope.branchId(),
                itemId);
        return rows.isEmpty() ? null : rows.get(0);
    }

    protected BigDecimal movementDelta(Scope scope, UUID itemId, String type) {
        List<BigDecimal> rows = jdbcTemplate.queryForList(
                "select quantity_delta from inventory_movements "
                        + "where branch_id = ? and inventory_item_id = ? and type = ?",
                BigDecimal.class,
                scope.branchId(),
                itemId,
                type);
        return rows.isEmpty() ? null : rows.get(0);
    }

    protected BigDecimal movementResulting(Scope scope, UUID itemId, String type) {
        List<BigDecimal> rows = jdbcTemplate.queryForList(
                "select resulting_quantity from inventory_movements "
                        + "where branch_id = ? and inventory_item_id = ? and type = ?",
                BigDecimal.class,
                scope.branchId(),
                itemId,
                type);
        return rows.isEmpty() ? null : rows.get(0);
    }

    protected long countReceipts(Scope scope) {
        return count("select count(*) from purchase_receipts where organization_id = ?", scope.fixture().orgId());
    }

    protected long countReceiptLines(Scope scope) {
        return count(
                "select count(*) from purchase_receipt_lines where organization_id = ?", scope.fixture().orgId());
    }

    protected long countMovements(Scope scope, String type) {
        return count(
                "select count(*) from inventory_movements where organization_id = ? and type = ?",
                scope.fixture().orgId(),
                type);
    }

    protected long auditActionCount(Scope scope, String action) {
        return count(
                "select count(*) from audit_events where organization_id = ? and action = ?",
                scope.fixture().orgId(),
                action);
    }

    protected String orderStatus(String orderId) {
        return jdbcTemplate.queryForObject(
                "select status from purchase_orders where id = ?", String.class, UUID.fromString(orderId));
    }

    protected long orderVersion(String orderId) {
        Long version = jdbcTemplate.queryForObject(
                "select version from purchase_orders where id = ?", Long.class, UUID.fromString(orderId));
        return version == null ? -1 : version;
    }

    protected long branchVersion(Scope scope) {
        Long version =
                jdbcTemplate.queryForObject("select version from branches where id = ?", Long.class, scope.branchId());
        return version == null ? -1 : version;
    }

    protected long itemVersion(UUID itemId) {
        Long version =
                jdbcTemplate.queryForObject("select version from inventory_items where id = ?", Long.class, itemId);
        return version == null ? -1 : version;
    }

    private long count(String sql, Object... args) {
        Long value = jdbcTemplate.queryForObject(sql, Long.class, args);
        return value == null ? 0 : value;
    }
}
