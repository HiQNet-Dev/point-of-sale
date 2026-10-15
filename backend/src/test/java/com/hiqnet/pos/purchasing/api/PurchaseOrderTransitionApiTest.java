package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Pruebas de integración de las transiciones de estado de una orden de compra
 * ({@code issue} y {@code cancel}) sobre la cadena de seguridad real (JWT + RBAC
 * en PostgreSQL) y PostgreSQL real.
 */
class PurchaseOrderTransitionApiTest extends AbstractAuditIntegrationTest {

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
        return setup(fixture, adminToken(fixture));
    }

    private Setup setup(RbacFixture fixture, String token) {
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item = inventoryItemService.create(
                fixture.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        return new Setup(fixture, token, branch.getId(), supplier.getId(), item.getId());
    }

    private String ordersUrl(Setup setup) {
        return BASE + setup.fixture().orgId() + "/branches/" + setup.branchId() + "/purchase-orders";
    }

    private String createOrder(Setup setup, String code, String token) throws Exception {
        MvcResult result = mockMvc.perform(post(ordersUrl(setup))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString())
                .get("purchaseOrder")
                .get("id")
                .asText();
    }

    private void addLine(Setup setup, String orderId, long expectedVersion, String token) throws Exception {
        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + ",\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"10.000000\"}"))
                .andExpect(status().isCreated());
    }

    private MvcResult issue(Setup setup, String orderId, long expectedVersion, String token) throws Exception {
        return mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + "}"))
                .andReturn();
    }

    private MvcResult cancel(Setup setup, String orderId, long expectedVersion, String token) throws Exception {
        return mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + expectedVersion + "}"))
                .andReturn();
    }

    private long countInventoryMovements(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    private long countBranchBalances(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from branch_inventory where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    @Test
    void issueTransitionsDraftToOrderedStampsActorAndTimeAndAudits() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-issue", setup.token());
        addLine(setup, orderId, 0, setup.token());
        long before = auditCount(setup.fixture().orgId());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("ORDERED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(2))
                .andExpect(jsonPath("$.purchaseOrder.issuedByUserId").value(setup.fixture().userId().toString()))
                .andExpect(jsonPath("$.purchaseOrder.issuedAt").exists())
                .andExpect(jsonPath("$.purchaseOrder.cancelledAt").doesNotExist())
                .andExpect(jsonPath("$.lines.length()").value(1));

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.issued");
        assertThat(latestAudit(setup.fixture().orgId()).branchId()).isEqualTo(setup.branchId());
        assertThat(metadataOf(latestAudit(setup.fixture().orgId()))).containsEntry("toStatus", "ORDERED");
        assertThat(auditCount(setup.fixture().orgId())).isEqualTo(before + 1);
        assertThat(countInventoryMovements(setup.fixture().orgId())).isZero();
        assertThat(countBranchBalances(setup.fixture().orgId())).isZero();
    }

    @Test
    void issueRejectsEmptyOrder() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-empty", setup.token());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_EMPTY"));

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.created");
    }

    @Test
    void issueIsRejectedFromOrderedOrCancelled() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-not-draft", setup.token());
        addLine(setup, orderId, 0, setup.token());
        issue(setup, orderId, 1, setup.token()).getResponse();

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_DRAFT"));

        Setup other = setup();
        String cancelledId = createOrder(other, "po-cancelled", other.token());
        cancel(other, cancelledId, 0, other.token());
        mockMvc.perform(post(ordersUrl(other) + "/" + cancelledId + "/issue")
                        .header("Authorization", bearer(other.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_DRAFT"));
    }

    @Test
    void issueRejectsInactiveSupplier() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-inactive-supplier", setup.token());
        addLine(setup, orderId, 0, setup.token());
        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/suppliers/" + setup.supplierId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("SUPPLIER_INACTIVE"));
    }

    @Test
    void issueRejectsInactiveInventoryItem() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-inactive-item", setup.token());
        addLine(setup, orderId, 0, setup.token());
        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/inventory-items/" + setup.itemId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_ITEM_INACTIVE"));
    }

    @Test
    void issueRejectsChangedBaseUnit() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-unit-changed", setup.token());
        addLine(setup, orderId, 0, setup.token());
        mockMvc.perform(patch(BASE + setup.fixture().orgId() + "/inventory-items/" + setup.itemId())
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"baseUnit\":\"UNIT\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_UNIT_CHANGED"));
    }

    @Test
    void issueRejectsStaleVersion() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-stale", setup.token());
        addLine(setup, orderId, 0, setup.token());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void cancelDraftSetsCancelledWithoutIssueDataAndAudits() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-cancel-draft", setup.token());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("CANCELLED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(1))
                .andExpect(jsonPath("$.purchaseOrder.cancelledByUserId").value(setup.fixture().userId().toString()))
                .andExpect(jsonPath("$.purchaseOrder.cancelledAt").exists())
                .andExpect(jsonPath("$.purchaseOrder.issuedAt").doesNotExist())
                .andExpect(jsonPath("$.purchaseOrder.issuedByUserId").doesNotExist());

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.cancelled");
        assertThat(metadataOf(latestAudit(setup.fixture().orgId()))).containsEntry("fromStatus", "DRAFT");
        assertThat(metadataOf(latestAudit(setup.fixture().orgId()))).containsEntry("toStatus", "CANCELLED");
    }

    @Test
    void cancelOrderedPreservesIssueHistory() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-cancel-ordered", setup.token());
        addLine(setup, orderId, 0, setup.token());
        issue(setup, orderId, 1, setup.token());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("CANCELLED"))
                .andExpect(jsonPath("$.purchaseOrder.version").value(3))
                .andExpect(jsonPath("$.purchaseOrder.issuedAt").exists())
                .andExpect(jsonPath("$.purchaseOrder.issuedByUserId").value(setup.fixture().userId().toString()))
                .andExpect(jsonPath("$.purchaseOrder.cancelledAt").exists())
                .andExpect(jsonPath("$.lines.length()").value(1));

        assertThat(latestAudit(setup.fixture().orgId()).action()).isEqualTo("purchase_order.cancelled");
        assertThat(countInventoryMovements(setup.fixture().orgId())).isZero();
        assertThat(countBranchBalances(setup.fixture().orgId())).isZero();
    }

    @Test
    void cancelIsRejectedWhenAlreadyCancelled() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-cancel-twice", setup.token());
        cancel(setup, orderId, 0, setup.token());

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_ALREADY_CANCELLED"));

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void transitionsRequireAuthentication() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-auth", setup.token());
        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/issue")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isUnauthorized());
        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/cancel")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void issueRequiresIssuePermissionNotJustWrite() throws Exception {
        RbacFixture writer = fixture();
        Setup writerSetup = setup(writer, null);
        createRoleAndAssign(writer, "order-writer", "Order Writer", "purchasing.order.read", "purchasing.order.write");
        String writerToken = login(writer);
        String orderId = createOrder(writerSetup, "po-write-only", writerToken);
        addLine(writerSetup, orderId, 0, writerToken);

        mockMvc.perform(post(ordersUrl(writerSetup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(writerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void issueWithoutCancelPermissionIsAllowedButCancelIsForbidden() throws Exception {
        RbacFixture issuer = fixture();
        Setup issuerSetup = setup(issuer, null);
        createRoleAndAssign(
                issuer,
                "order-issuer",
                "Order Issuer",
                "purchasing.order.read",
                "purchasing.order.write",
                "purchasing.order.issue");
        String issuerToken = login(issuer);
        String orderId = createOrder(issuerSetup, "po-issue-only", issuerToken);
        addLine(issuerSetup, orderId, 0, issuerToken);

        mockMvc.perform(post(ordersUrl(issuerSetup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(issuerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("ORDERED"));

        mockMvc.perform(post(ordersUrl(issuerSetup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(issuerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void cancelWithoutIssuePermissionIsAllowedButIssueIsForbidden() throws Exception {
        RbacFixture canceller = fixture();
        Setup cancellerSetup = setup(canceller, null);
        createRoleAndAssign(
                canceller,
                "order-canceller",
                "Order Canceller",
                "purchasing.order.read",
                "purchasing.order.write",
                "purchasing.order.cancel");
        String cancellerToken = login(canceller);
        String orderId = createOrder(cancellerSetup, "po-cancel-only", cancellerToken);

        mockMvc.perform(post(ordersUrl(cancellerSetup) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(cancellerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(post(ordersUrl(cancellerSetup) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(cancellerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.purchaseOrder.status").value("CANCELLED"));
    }

    @Test
    void transitionsAreIsolatedAcrossOrganizationAndBranch() throws Exception {
        Setup setupA = setup();
        Setup setupB = setup();
        String orderId = createOrder(setupA, "po-isolated", setupA.token());
        addLine(setupA, orderId, 0, setupA.token());

        mockMvc.perform(post(BASE + setupB.fixture().orgId() + "/branches/" + setupB.branchId()
                        + "/purchase-orders/" + orderId + "/issue")
                        .header("Authorization", bearer(setupB.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_FOUND"));

        Branch secondBranch = branchService.create(setupA.fixture().orgId(), "Norte", "sucursal-norte", null);
        mockMvc.perform(post(BASE + setupA.fixture().orgId() + "/branches/" + secondBranch.getId()
                        + "/purchase-orders/" + orderId + "/cancel")
                        .header("Authorization", bearer(setupA.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_FOUND"));
    }

    @Test
    void issuedOrderIsImmutable() throws Exception {
        Setup setup = setup();
        String orderId = createOrder(setup, "po-immutable", setup.token());
        addLine(setup, orderId, 0, setup.token());
        issue(setup, orderId, 1, setup.token());

        mockMvc.perform(patch(ordersUrl(setup) + "/" + orderId)
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"supplierId\":\"" + setup.supplierId() + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_DRAFT"));

        mockMvc.perform(post(ordersUrl(setup) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(setup.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"inventoryItemId\":\"" + setup.itemId()
                                + "\",\"quantity\":\"1.000000\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PURCHASE_ORDER_NOT_DRAFT"));
    }
}
