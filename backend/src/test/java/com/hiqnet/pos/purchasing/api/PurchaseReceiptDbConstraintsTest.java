package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
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
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Pruebas de que la integridad de la recepción la impone PostgreSQL, no solo el
 * código Java: la clave de idempotencia es única por organización y orden, y cada
 * línea de recepción origina como máximo un movimiento en el ledger.
 */
class PurchaseReceiptDbConstraintsTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private record Setup(RbacFixture fixture, String token, UUID branchId, UUID supplierId, UUID itemId) {
    }

    private Setup setup() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item = inventoryItemService.create(
                fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        return new Setup(fixture, token, branch.getId(), supplier.getId(), item.getId());
    }

    private String createOrder(Setup setup, String code) throws Exception {
        MvcResult created = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper
                .readTree(created.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
    }

    @Test
    void duplicateIdempotencyKeyIsRejectedByPostgres() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-dup-key");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> {
                    insertReceipt(setup, orderId, "same-key");
                    insertReceipt(setup, orderId, "same-key");
                }))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void duplicateMovementPerReceiptLineIsRejectedByPostgres() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-dup-line");
        MvcResult line = mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + setup.itemId() + "\",\"quantity\":\"10.000000\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        UUID lineId = UUID.fromString(objectMapper
                .readTree(line.getResponse().getContentAsString())
                .get("lines")
                .get(0)
                .get("id")
                .asText());
        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk());

        MvcResult receipt = mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/receipts")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"idempotencyKey\":\"k-db\",\"lines\":[{\"purchaseOrderLineId\":\""
                                + lineId + "\",\"quantity\":\"10.000000\"}]}"))
                .andExpect(status().isCreated())
                .andReturn();
        UUID receiptId = UUID.fromString(objectMapper
                .readTree(receipt.getResponse().getContentAsString())
                .get("purchaseReceipt")
                .get("id")
                .asText());
        UUID receiptLineId = jdbcTemplate.queryForObject(
                "select id from purchase_receipt_lines where purchase_receipt_id = ?", UUID.class, receiptId);

        TransactionTemplate transaction = new TransactionTemplate(transactionManager);
        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> {
                    insertReceiptMovement(setup, receiptLineId);
                    insertReceiptMovement(setup, receiptLineId);
                }))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    private void insertReceipt(Setup setup, String orderId, String key) {
        jdbcTemplate.update(
                "insert into purchase_receipts "
                        + "(id,organization_id,branch_id,purchase_order_id,idempotency_key,payload_fingerprint,"
                        + "received_by_user_id,received_at,created_at) values (?,?,?,?,?,?,?,now(),now())",
                UUID.randomUUID(),
                setup.fixture().orgId(),
                setup.branchId(),
                UUID.fromString(orderId),
                key,
                "0".repeat(64),
                setup.fixture().userId());
    }

    private void insertReceiptMovement(Setup setup, UUID receiptLineId) {
        jdbcTemplate.update(
                "insert into inventory_movements "
                        + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                        + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                        + "values (?,?,?,?,'PURCHASE_RECEIPT',1.000000,100.000000,?,null,?,now())",
                UUID.randomUUID(),
                setup.fixture().orgId(),
                setup.branchId(),
                setup.itemId(),
                setup.fixture().userId(),
                receiptLineId);
    }

    @Test
    void purchaseReceiptMovementWithoutLineReferenceIsRejected() throws Exception {
        Setup setup = setup();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "insert into inventory_movements "
                                + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                                + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                                + "values (?,?,?,?,'PURCHASE_RECEIPT',1.000000,1.000000,?,null,null,now())",
                        UUID.randomUUID(),
                        setup.fixture().orgId(),
                        setup.branchId(),
                        setup.itemId(),
                        setup.fixture().userId())))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void purchaseReceiptMovementWithSaleReferenceIsRejected() throws Exception {
        Setup setup = setup();
        UUID receiptLineId = createReceiptLine(setup, "po-movement-sale", "1.000000");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "insert into inventory_movements "
                                + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                                + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                                + "values (?,?,?,?,'PURCHASE_RECEIPT',1.000000,1.000000,?,?,?,now())",
                        UUID.randomUUID(),
                        setup.fixture().orgId(),
                        setup.branchId(),
                        setup.itemId(),
                        setup.fixture().userId(),
                        UUID.randomUUID(),
                        receiptLineId)))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void nonPositivePurchaseReceiptMovementDeltaIsRejected() throws Exception {
        Setup setup = setup();
        UUID receiptLineId = createReceiptLine(setup, "po-movement-delta", "1.000000");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "insert into inventory_movements "
                                + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                                + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                                + "values (?,?,?,?,'PURCHASE_RECEIPT',0.000000,1.000000,?,null,?,now())",
                        UUID.randomUUID(),
                        setup.fixture().orgId(),
                        setup.branchId(),
                        setup.itemId(),
                        setup.fixture().userId(),
                        receiptLineId)))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void nonReceiptMovementsDoNotRequireALineReference() throws Exception {
        Setup setup = setup();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        transaction.executeWithoutResult(status -> jdbcTemplate.update(
                "insert into inventory_movements "
                        + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                        + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                        + "values (?,?,?,?,'OPENING',5.000000,5.000000,?,null,null,now())",
                UUID.randomUUID(),
                setup.fixture().orgId(),
                setup.branchId(),
                setup.itemId(),
                setup.fixture().userId()));

        assertThat(jdbcTemplate.queryForObject(
                        "select count(*) from inventory_movements where organization_id = ? and type = 'OPENING'",
                        Long.class,
                        setup.fixture().orgId()))
                .isEqualTo(1L);
    }

    @Test
    void inventoryMovementsAreAppendOnly() throws Exception {
        Setup setup = setup();
        UUID movementId = insertOpeningMovement(setup);
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "update inventory_movements set reason = 'tampered' where id = ?", movementId)))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> jdbcTemplate.update("delete from inventory_movements where id = ?", movementId)))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void orderStatusCheckAllowsPartiallyReceivedAndRejectsUnknown() throws Exception {
        Setup setup = setup();
        String orderId = issuedOrderWithLine(setup, "po-status-check", "10.000000");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        // PARTIALLY_RECEIVED (18 caracteres) cabe en la columna ampliada y conserva la emisión.
        transaction.executeWithoutResult(status -> jdbcTemplate.update(
                "update purchase_orders set status = 'PARTIALLY_RECEIVED' where id = ?", UUID.fromString(orderId)));
        assertThat(jdbcTemplate.queryForObject(
                        "select status from purchase_orders where id = ?", String.class, UUID.fromString(orderId)))
                .isEqualTo("PARTIALLY_RECEIVED");

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "update purchase_orders set status = 'PARTIAL' where id = ?", UUID.fromString(orderId))))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void receiptBranchMustMatchItsOrder() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-wrong-branch");
        UUID otherBranchId = branchService
                .create(setup.fixture().orgId(), "Norte", "sucursal-norte", null)
                .getId();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "insert into purchase_receipts "
                                + "(id,organization_id,branch_id,purchase_order_id,idempotency_key,payload_fingerprint,"
                                + "received_by_user_id,received_at,created_at) values (?,?,?,?,?,?,?,now(),now())",
                        UUID.randomUUID(),
                        setup.fixture().orgId(),
                        otherBranchId,
                        UUID.fromString(orderId),
                        "k-wrong-branch",
                        "0".repeat(64),
                        setup.fixture().userId())))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void receiptMovementWithDifferentItemThanItsLineIsRejected() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-identity-item", "10.000000");
        UUID receiptLineId = receiptLineViaSql(setup, setup.branchId(), order, "po-identity-item");
        UUID otherItemId = inventoryItemService
                .create(setup.fixture().orgId(), "azucar", "Azucar", null, InventoryBaseUnit.GRAM, true)
                .getId();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        // Mismo alcance (organización, sucursal) pero artículo distinto al de la línea: rechazado.
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> insertReceiptMovementRow(setup, receiptLineId, otherItemId)))
                .isInstanceOf(DataIntegrityViolationException.class);

        // Con el artículo de la línea, la fila entra en el ledger.
        transaction.executeWithoutResult(status -> insertReceiptMovementRow(setup, receiptLineId, setup.itemId()));
        assertThat(countReceiptMovements(setup)).isEqualTo(1L);
    }

    @Test
    void receiptMovementReferencingAnotherOrganizationLineIsRejected() throws Exception {
        Setup first = setup();
        Setup second = setup();
        IssuedOrder foreignOrder = issuedOrder(second, "po-identity-org", "10.000000");
        UUID foreignLineId = receiptLineViaSql(second, second.branchId(), foreignOrder, "po-identity-org");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> insertReceiptMovementRow(first, foreignLineId, first.itemId())))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void receiptMovementReferencingAnotherBranchLineIsRejected() throws Exception {
        Setup setup = setup();
        Branch other = branchService.create(setup.fixture().orgId(), "Norte", "sucursal-norte", null);
        IssuedOrder orderOnOther = issuedOrderOnBranch(setup, other.getId(), "po-identity-branch", "10.000000");
        UUID otherBranchLineId = receiptLineViaSql(setup, other.getId(), orderOnOther, "po-identity-branch");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        // La línea pertenece a otra sucursal de la misma organización: rechazado.
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> insertReceiptMovementRow(setup, otherBranchLineId, setup.itemId())))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void receivedLineCannotBeModifiedOrDeleted() throws Exception {
        Setup setup = setup();
        UUID receiptLineId = createReceiptLine(setup, "po-immutable-line", "1.000000");
        UUID otherItemId = inventoryItemService
                .create(setup.fixture().orgId(), "sal", "Sal", null, InventoryBaseUnit.GRAM, true)
                .getId();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "update purchase_receipt_lines set quantity_received = 2.000000 where id = ?", receiptLineId)))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "update purchase_receipt_lines set inventory_item_id = ? where id = ?", otherItemId, receiptLineId)))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> jdbcTemplate.update("delete from purchase_receipt_lines where id = ?", receiptLineId)))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void confirmedReceiptCannotBeModifiedOrDeleted() throws Exception {
        Setup setup = setup();
        UUID receiptLineId = createReceiptLine(setup, "po-immutable-header", "1.000000");
        UUID receiptId = receiptHeaderIdForLine(receiptLineId);
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> transaction.executeWithoutResult(status -> jdbcTemplate.update(
                        "update purchase_receipts set payload_fingerprint = repeat('a', 64) where id = ?", receiptId)))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> jdbcTemplate.update("delete from purchase_receipts where id = ?", receiptId)))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void validReceiptHeaderLineAndMovementCanBeInsertedDirectly() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "po-valid-insert", "10.000000");
        UUID receiptId = UUID.randomUUID();
        UUID lineId = UUID.randomUUID();
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        transaction.executeWithoutResult(status -> {
            insertReceiptRow(receiptId, setup, setup.branchId(), order.orderId(), "k-valid-insert");
            insertReceiptLineRow(lineId, setup, setup.branchId(), receiptId, order.orderId(), order.lineId(), setup.itemId());
            insertReceiptMovementRow(setup, lineId, setup.itemId());
        });

        assertThat(jdbcTemplate.queryForObject(
                        "select count(*) from purchase_receipts where id = ?", Long.class, receiptId))
                .isEqualTo(1L);
        assertThat(jdbcTemplate.queryForObject(
                        "select count(*) from purchase_receipt_lines where purchase_receipt_id = ?",
                        Long.class,
                        receiptId))
                .isEqualTo(1L);
        assertThat(countReceiptMovements(setup)).isEqualTo(1L);
    }

    @Test
    void confirmedReceiptIdempotencyKeyCannotBeReusedButReplayRemainsAvailable() throws Exception {
        Setup setup = setup();
        IssuedOrder order = issuedOrder(setup, "k-idem-reuse", "1.000000");
        UUID receiptId = postReceipt(setup, setup.branchId(), order, "k-idem-reuse", "1.000000");
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);

        // Otra fila con la misma (organización, orden, clave): PostgreSQL la rechaza.
        assertThatThrownBy(() -> transaction.executeWithoutResult(
                        status -> insertReceiptRow(UUID.randomUUID(), setup, setup.branchId(), order.orderId(), "k-idem-reuse")))
                .isInstanceOf(DataIntegrityViolationException.class);

        // El reintento idéntico por la API sigue devolviendo el resultado original sin efectos nuevos.
        mockMvc.perform(post(ordersUrl(setup) + "/" + order.orderId() + "/receipts")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"idempotencyKey\":\"k-idem-reuse\",\"lines\":[{\"purchaseOrderLineId\":\""
                                + order.lineId() + "\",\"quantity\":\"1.000000\"}]}"))
                .andExpect(status().isOk());

        assertThat(jdbcTemplate.queryForObject(
                        "select count(*) from purchase_receipts where id = ?", Long.class, receiptId))
                .isEqualTo(1L);
        assertThat(countReceiptMovements(setup)).isEqualTo(1L);
    }

    private record IssuedOrder(String orderId, UUID lineId) {}

    private String ordersUrl(Setup setup) {
        return ordersUrl(setup, setup.branchId());
    }

    private String ordersUrl(Setup setup, UUID branchId) {
        return BASE + setup.fixture().orgId() + "/branches/" + branchId + "/purchase-orders";
    }

    private String issuedOrderWithLine(Setup setup, String code, String quantity) throws Exception {
        return issuedOrder(setup, code, quantity).orderId();
    }

    private IssuedOrder issuedOrder(Setup setup, String code, String quantity) throws Exception {
        return issuedOrderOnBranch(setup, setup.branchId(), code, quantity);
    }

    private IssuedOrder issuedOrderOnBranch(Setup setup, UUID branchId, String code, String quantity) throws Exception {
        String orderId = createOrderOnBranch(setup, branchId, code);
        MvcResult line = mockMvc.perform(post(ordersUrl(setup, branchId) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"" + quantity + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        UUID lineId = UUID.fromString(objectMapper
                .readTree(line.getResponse().getContentAsString())
                .get("lines")
                .get(0)
                .get("id")
                .asText());
        mockMvc.perform(post(ordersUrl(setup, branchId) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk());
        return new IssuedOrder(orderId, lineId);
    }

    private String createOrderOnBranch(Setup setup, UUID branchId, String code) throws Exception {
        MvcResult created = mockMvc.perform(post(ordersUrl(setup, branchId))
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return receiptIdField(created, "purchaseOrder");
    }

    private UUID createReceiptLine(Setup setup, String code, String quantity) throws Exception {
        return createReceiptLineOnBranch(setup, setup.branchId(), code, quantity);
    }

    private UUID createReceiptLineOnBranch(Setup setup, UUID branchId, String code, String quantity) throws Exception {
        IssuedOrder order = issuedOrderOnBranch(setup, branchId, code, quantity);
        UUID receiptId = postReceipt(setup, branchId, order, code, quantity);
        return jdbcTemplate.queryForObject(
                "select id from purchase_receipt_lines where purchase_receipt_id = ?", UUID.class, receiptId);
    }

    private UUID postReceipt(Setup setup, UUID branchId, IssuedOrder order, String key, String quantity)
            throws Exception {
        MvcResult receipt = mockMvc.perform(post(ordersUrl(setup, branchId) + "/" + order.orderId() + "/receipts")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"idempotencyKey\":\"" + key
                                + "\",\"lines\":[{\"purchaseOrderLineId\":\"" + order.lineId()
                                + "\",\"quantity\":\"" + quantity + "\"}]}"))
                .andExpect(status().isCreated())
                .andReturn();
        return UUID.fromString(receiptIdField(receipt, "purchaseReceipt"));
    }

    private String receiptIdField(MvcResult result, String node) throws Exception {
        return objectMapper
                .readTree(result.getResponse().getContentAsString())
                .get(node)
                .get("id")
                .asText();
    }

    private UUID receiptHeaderIdForLine(UUID receiptLineId) {
        return jdbcTemplate.queryForObject(
                "select purchase_receipt_id from purchase_receipt_lines where id = ?", UUID.class, receiptLineId);
    }

    private long countReceiptMovements(Setup setup) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ? and type = 'PURCHASE_RECEIPT'",
                Long.class,
                setup.fixture().orgId());
        return count == null ? 0 : count;
    }

    private UUID receiptLineViaSql(Setup setup, UUID branchId, IssuedOrder order, String key) {
        UUID receiptId = UUID.randomUUID();
        UUID lineId = UUID.randomUUID();
        insertReceiptRow(receiptId, setup, branchId, order.orderId(), key);
        insertReceiptLineRow(lineId, setup, branchId, receiptId, order.orderId(), order.lineId(), setup.itemId());
        return lineId;
    }

    private void insertReceiptRow(UUID receiptId, Setup setup, UUID branchId, String orderId, String key) {
        jdbcTemplate.update(
                "insert into purchase_receipts "
                        + "(id,organization_id,branch_id,purchase_order_id,idempotency_key,payload_fingerprint,"
                        + "received_by_user_id,received_at,created_at) values (?,?,?,?,?,?,?,now(),now())",
                receiptId,
                setup.fixture().orgId(),
                branchId,
                UUID.fromString(orderId),
                key,
                "0".repeat(64),
                setup.fixture().userId());
    }

    private void insertReceiptLineRow(
            UUID lineId, Setup setup, UUID branchId, UUID receiptId, String orderId, UUID orderLineId, UUID itemId) {
        jdbcTemplate.update(
                "insert into purchase_receipt_lines "
                        + "(id,organization_id,branch_id,purchase_receipt_id,purchase_order_id,purchase_order_line_id,"
                        + "inventory_item_id,quantity_received,created_at) values (?,?,?,?,?,?,?,1.000000,now())",
                lineId,
                setup.fixture().orgId(),
                branchId,
                receiptId,
                UUID.fromString(orderId),
                orderLineId,
                itemId);
    }

    private void insertReceiptMovementRow(Setup setup, UUID receiptLineId, UUID itemId) {
        jdbcTemplate.update(
                "insert into inventory_movements "
                        + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                        + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                        + "values (?,?,?,?,'PURCHASE_RECEIPT',1.000000,1.000000,?,null,?,now())",
                UUID.randomUUID(),
                setup.fixture().orgId(),
                setup.branchId(),
                itemId,
                setup.fixture().userId(),
                receiptLineId);
    }

    private UUID insertOpeningMovement(Setup setup) {
        UUID movementId = UUID.randomUUID();
        jdbcTemplate.update(
                "insert into inventory_movements "
                        + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,"
                        + "actor_user_id,sale_id,purchase_receipt_line_id,occurred_at) "
                        + "values (?,?,?,?,'OPENING',5.000000,5.000000,?,null,null,now())",
                movementId,
                setup.fixture().orgId(),
                setup.branchId(),
                setup.itemId(),
                setup.fixture().userId());
        return movementId;
    }
}
