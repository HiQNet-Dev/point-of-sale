package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.application.UpdateInventoryItemCommand;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.purchasing.application.CreatePurchaseOrderLineCommand;
import com.hiqnet.pos.purchasing.application.PurchaseOrderDetail;
import com.hiqnet.pos.purchasing.application.PurchaseOrderService;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.application.UpdateSupplierCommand;
import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.purchasing.domain.SupplierRepository;
import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import com.hiqnet.pos.shared.error.BusinessException;
import jakarta.persistence.EntityManager;
import java.math.BigDecimal;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Consistencia transaccional de la emisión de órdenes de compra (la carrera
 * TOCTOU de emisión) sobre PostgreSQL real.
 *
 * <p>La emisión valida el proveedor y los artículos con bloqueo pesimista en el
 * orden {@code PurchaseOrder → Supplier → InventoryItem}; los escritores
 * administrativos ({@code SupplierService.update}, {@code InventoryItemService
 * .update}) toman esos mismos bloqueos de fila. Aquí se prueba (1) que la emisión
 * adquiere realmente cada bloqueo, (2) que no decide sobre un estado obsoleto del
 * Persistence Context, (3) que ambas órdenes de adquisición quedan determinadas
 * (una transacción bloqueada agota su {@code lock_timeout}, SQLState {@code 55P03})
 * y (4) que emisión y desactivación/edición concurrentes dejan un resultado
 * coherente, sin estado parcial ni movimientos de inventario.</p>
 *
 * <p>Las carreras emisión vs emisión, emisión vs cancelación y emisión vs
 * modificación de partidas ya están cubiertas por {@link PurchaseOrderConcurrencyTest}
 * y no se duplican aquí.</p>
 */
class PurchaseOrderIssuanceConsistencyTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private PurchaseOrderService purchaseOrderService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierRepository supplierRepository;

    @Autowired
    private InventoryItemRepository inventoryItemRepository;

    @Autowired
    private EntityManager entityManager;

    private record Scenario(
            RbacFixture fixture,
            UUID branchId,
            UUID supplierId,
            List<UUID> itemIds,
            UUID orderId,
            UUID actorId,
            long expectedVersion) {
    }

    private record HttpResult(int status, String code) {
    }

    private Scenario scenario(int itemCount) {
        RbacFixture fixture = fixture();
        UUID orgId = fixture.orgId();
        Branch branch = branchService.create(orgId, "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                orgId, "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        List<UUID> items = new ArrayList<>();
        for (int index = 0; index < itemCount; index++) {
            items.add(inventoryItemService
                    .create(orgId, "item-" + index, "Item " + index, null, InventoryBaseUnit.UNIT, false)
                    .getId());
        }
        PurchaseOrderDetail order =
                purchaseOrderService.create(orgId, branch.getId(), "po-consistency", supplier.getId(), fixture.userId());
        long expectedVersion = order.order().getVersion();
        for (UUID itemId : items) {
            PurchaseOrderDetail updated = purchaseOrderService.addLine(
                    orgId,
                    branch.getId(),
                    order.order().getId(),
                    new CreatePurchaseOrderLineCommand(expectedVersion, itemId, new BigDecimal("1.000000"), null));
            expectedVersion = updated.order().getVersion();
        }
        return new Scenario(
                fixture,
                branch.getId(),
                supplier.getId(),
                items,
                order.order().getId(),
                fixture.userId(),
                expectedVersion);
    }

    // --- Deterministic proofs that the emission acquires each row lock ----------

    @Test
    void issueAcquiresTheSupplierRowLockBeforeValidating() throws Exception {
        Scenario scenario = scenario(1);
        CountDownLatch supplierLocked = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        Thread deactivator = new Thread(() -> new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            supplierService.update(
                    scenario.fixture().orgId(),
                    scenario.supplierId(),
                    new UpdateSupplierCommand(0, null, null, null, null, null, null, null, SupplierStatus.INACTIVE));
            supplierLocked.countDown();
            await(release);
        }));

        deactivator.start();
        assertThat(supplierLocked.await(15, TimeUnit.SECONDS)).isTrue();
        Throwable failure = issueWithLockTimeout(scenario, "750ms");
        release.countDown();
        deactivator.join();

        // La emisión esperó la fila del proveedor y agotó el lock_timeout: prueba
        // de que toma el bloqueo en lugar de leer un estado no serializado.
        assertThat(sqlStateOf(failure)).isEqualTo("55P03");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
        assertThat(issuedAuditCount(scenario.fixture().orgId())).isZero();
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    @Test
    void issueAcquiresTheInventoryItemRowLockBeforeValidating() throws Exception {
        Scenario scenario = scenario(1);
        CountDownLatch itemLocked = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        Thread deactivator = new Thread(() -> new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            inventoryItemService.update(
                    scenario.fixture().orgId(),
                    scenario.itemIds().get(0),
                    new UpdateInventoryItemCommand(0, null, null, null, null, InventoryItemStatus.INACTIVE));
            itemLocked.countDown();
            await(release);
        }));

        deactivator.start();
        assertThat(itemLocked.await(15, TimeUnit.SECONDS)).isTrue();
        Throwable failure = issueWithLockTimeout(scenario, "750ms");
        release.countDown();
        deactivator.join();

        assertThat(sqlStateOf(failure)).isEqualTo("55P03");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    @Test
    void emissionHoldsTheSupplierLockSoAConcurrentDeactivationWaits() throws Exception {
        Scenario scenario = scenario(1);
        UUID orgId = scenario.fixture().orgId();
        UUID itemId = scenario.itemIds().get(0);
        String token = adminToken(scenario.fixture());

        // Retenemos el bloqueo del artículo para forzar a la emisión a detenerse
        // DESPUÉS de adquirir los bloqueos de la orden y del proveedor.
        CountDownLatch itemLocked = new CountDownLatch(1);
        CountDownLatch releaseItem = new CountDownLatch(1);
        Thread itemHolder = new Thread(() -> new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            inventoryItemRepository.findByIdAndOrganizationIdForUpdate(itemId, orgId).orElseThrow();
            itemLocked.countDown();
            await(releaseItem);
        }));
        itemHolder.start();
        assertThat(itemLocked.await(15, TimeUnit.SECONDS)).isTrue();

        AtomicReference<HttpResult> issueResult = new AtomicReference<>();
        AtomicReference<Throwable> issueFailure = new AtomicReference<>();
        Thread emission = new Thread(() -> {
            try {
                issueResult.set(issueHttp(scenario, token));
            } catch (Throwable failure) {
                issueFailure.set(failure);
            }
        });
        emission.start();

        // Determinista: mientras la emisión aparece como waiter del artículo ya
        // sostiene el bloqueo del proveedor; una desactivación concurrente debe por
        // tanto esperar en esa fila y agotar su lock_timeout.
        awaitBlockingOnRelation("inventory_items", 15);
        Throwable failure = deactivateSupplierWithLockTimeout(scenario, "750ms");
        assertThat(sqlStateOf(failure)).isEqualTo("55P03");

        releaseItem.countDown();
        emission.join(30_000);
        itemHolder.join();
        assertThat(issueFailure.get()).isNull();
        assertThat(issueResult.get().status()).isEqualTo(200);

        assertThat(orderStatus(scenario.orderId())).isEqualTo("ORDERED");
        assertThat(supplierStatus(scenario.supplierId())).isEqualTo("ACTIVE");
        assertThat(issuedAuditCount(orgId)).isEqualTo(1);
        assertThat(movementCount(orgId)).isZero();
        assertThat(balanceCount(orgId)).isZero();
    }

    @Test
    void issueReadsSupplierStatusFreshEvenWhenTheSupplierIsAlreadyManaged() throws Exception {
        Scenario scenario = scenario(1);
        UUID orgId = scenario.fixture().orgId();
        UUID supplierId = scenario.supplierId();
        CountDownLatch deactivated = new CountDownLatch(1);
        CountDownLatch proceed = new CountDownLatch(1);
        Thread deactivator = new Thread(() -> new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            supplierService.update(
                    orgId,
                    supplierId,
                    new UpdateSupplierCommand(0, null, null, null, null, null, null, null, SupplierStatus.INACTIVE));
            deactivated.countDown();
            await(proceed);
        }));

        deactivator.start();
        assertThat(deactivated.await(15, TimeUnit.SECONDS)).isTrue();

        AtomicReference<SupplierStatus> managedStatus = new AtomicReference<>();
        AtomicReference<String> lockedStatus = new AtomicReference<>();
        new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            // La orden carga su proveedor de forma EAGER: aquí se reproduce esa
            // instancia administrada antes de que la desactivación confirme.
            Supplier managed = entityManager.find(Supplier.class, supplierId);
            managedStatus.set(managed.getStatus());
            proceed.countDown();
            join(deactivator);
            // La lectura bloqueada de la emisión es escalar y nativa: ve el estado
            // vigente aunque la instancia administrada siga obsoleta.
            lockedStatus.set(supplierRepository.lockStatusByIdAndOrganizationId(supplierId, orgId));
            assertThat(managed.getStatus()).isEqualTo(SupplierStatus.ACTIVE);
        });
        deactivator.join();

        assertThat(managedStatus.get()).isEqualTo(SupplierStatus.ACTIVE);
        assertThat(lockedStatus.get()).isEqualTo("INACTIVE");
    }

    // --- Regression: the emission never decides on a stale Persistence Context --

    @Test
    void issueRejectsSupplierDeactivatedAfterTheOrderWasLoadedInTheSameTransaction() {
        Scenario scenario = scenario(1);
        UUID orgId = scenario.fixture().orgId();
        UUID supplierId = scenario.supplierId();
        Throwable failure = null;

        try {
            new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
                // Reproduce lo que hace lockOrder: carga la orden y su proveedor EAGER
                // en el Persistence Context antes de validar la emisión.
                entityManager.find(PurchaseOrder.class, scenario.orderId());
                Thread deactivator = new Thread(() -> new TransactionTemplate(transactionManager)
                        .executeWithoutResult(inner -> supplierService.update(
                                orgId,
                                supplierId,
                                new UpdateSupplierCommand(
                                        0, null, null, null, null, null, null, null, SupplierStatus.INACTIVE))));
                deactivator.start();
                join(deactivator);
                purchaseOrderService.issue(
                        orgId,
                        scenario.branchId(),
                        scenario.orderId(),
                        scenario.expectedVersion(),
                        scenario.actorId());
            });
        } catch (Throwable thrown) {
            failure = thrown;
        }

        // La emisión debe observar el estado vigente del proveedor aunque la instancia
        // administrada siga en ACTIVE; con la comprobación de entidad antigua emitiría.
        assertThat(failure).isInstanceOf(BusinessException.class);
        assertThat(((BusinessException) failure).getCode()).isEqualTo("SUPPLIER_INACTIVE");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
        assertThat(issuedAuditCount(orgId)).isZero();
    }

    @Test
    void issueRejectsItemDeactivatedAfterTheItemWasLoadedInTheSameTransaction() {
        Scenario scenario = scenario(1);
        UUID orgId = scenario.fixture().orgId();
        UUID itemId = scenario.itemIds().get(0);
        Throwable failure = null;

        try {
            new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
                // Precarga una instancia administrada del artículo en el Persistence
                // Context antes del bloqueo de emisión.
                inventoryItemRepository.findByIdAndOrganizationId(itemId, orgId).orElseThrow();
                Thread deactivator = new Thread(() -> new TransactionTemplate(transactionManager)
                        .executeWithoutResult(inner -> inventoryItemService.update(
                                orgId,
                                itemId,
                                new UpdateInventoryItemCommand(
                                        0, null, null, null, null, InventoryItemStatus.INACTIVE))));
                deactivator.start();
                join(deactivator);
                purchaseOrderService.issue(
                        orgId,
                        scenario.branchId(),
                        scenario.orderId(),
                        scenario.expectedVersion(),
                        scenario.actorId());
            });
        } catch (Throwable thrown) {
            failure = thrown;
        }

        assertThat(failure).isInstanceOf(BusinessException.class);
        assertThat(((BusinessException) failure).getCode()).isEqualTo("INVENTORY_ITEM_INACTIVE");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
        assertThat(issuedAuditCount(orgId)).isZero();
    }

    @Test
    void issueRejectsLineWhoseCapturedBaseUnitNoLongerMatchesTheItem() {
        Scenario scenario = scenario(1);
        UUID orgId = scenario.fixture().orgId();
        jdbcTemplate.update(
                "update purchase_order_lines set unit_snapshot = 'GRAM' "
                        + "where purchase_order_id = ? and inventory_item_id = ?",
                scenario.orderId(), scenario.itemIds().get(0));

        Throwable failure = null;
        try {
            purchaseOrderService.issue(
                    orgId,
                    scenario.branchId(),
                    scenario.orderId(),
                    scenario.expectedVersion(),
                    scenario.actorId());
        } catch (Throwable thrown) {
            failure = thrown;
        }

        // El artículo sigue ACTIVE pero su unidad base vigente (UNIT) ya no coincide
        // con el snapshot capturado (GRAM): la emisión debe rechazarlo.
        assertThat(failure).isInstanceOf(BusinessException.class);
        assertThat(((BusinessException) failure).getCode()).isEqualTo("PURCHASE_ORDER_UNIT_CHANGED");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
        assertThat(issuedAuditCount(orgId)).isZero();
    }

    // --- Scenario A: issue vs supplier deactivation ----------------------------

    @Test
    void issueAndSupplierDeactivationSerializeWithoutStaleActiveSupplier() throws Exception {
        Scenario scenario = scenario(1);
        String token = adminToken(scenario.fixture());

        List<HttpResult> results = runTwo(
                () -> issueHttp(scenario, token),
                () -> deactivateSupplierHttp(scenario, token));

        HttpResult issue = results.get(0);
        HttpResult deactivation = results.get(1);
        assertThat(deactivation.status()).isEqualTo(200);
        assertThat(issue.status()).isIn(200, 409);
        assertThat(supplierStatus(scenario.supplierId())).isEqualTo("INACTIVE");
        if (issue.status() == 200) {
            assertThat(orderStatus(scenario.orderId())).isEqualTo("ORDERED");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isEqualTo(1);
        } else {
            assertThat(issue.code()).isEqualTo("SUPPLIER_INACTIVE");
            assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isZero();
        }
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    // --- Scenario B: issue vs inventory item deactivation ----------------------

    @Test
    void issueAndInventoryItemDeactivationSerializeWithoutStaleActiveItem() throws Exception {
        Scenario scenario = scenario(1);
        String token = adminToken(scenario.fixture());

        List<HttpResult> results = runTwo(
                () -> issueHttp(scenario, token),
                () -> deactivateItemHttp(scenario, token, scenario.itemIds().get(0)));

        HttpResult issue = results.get(0);
        assertThat(results.get(1).status()).isEqualTo(200);
        assertThat(itemStatus(scenario.itemIds().get(0))).isEqualTo("INACTIVE");
        assertThat(issue.status()).isIn(200, 409);
        if (issue.status() == 200) {
            assertThat(orderStatus(scenario.orderId())).isEqualTo("ORDERED");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isEqualTo(1);
        } else {
            assertThat(issue.code()).isEqualTo("INVENTORY_ITEM_INACTIVE");
            assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isZero();
        }
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    // --- Scenario C: issue vs base unit change (policy forbids the change) -----

    @Test
    void issueAndBaseUnitChangeRaceKeepsTheCapturedUnit() throws Exception {
        Scenario scenario = scenario(1);
        String token = adminToken(scenario.fixture());
        UUID itemId = scenario.itemIds().get(0);

        List<HttpResult> results = runTwo(
                () -> issueHttp(scenario, token),
                () -> changeBaseUnitHttp(scenario, token, itemId, "GRAM"));

        assertThat(results.get(0).status()).isEqualTo(200);
        assertThat(results.get(1).status()).isEqualTo(409);
        assertThat(results.get(1).code()).isEqualTo("INVENTORY_ITEM_UNIT_LOCKED");
        assertThat(itemBaseUnit(itemId)).isEqualTo("UNIT");
        assertThat(orderStatus(scenario.orderId())).isEqualTo("ORDERED");
        assertThat(issuedAuditCount(scenario.fixture().orgId())).isEqualTo(1);
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    // --- Scenario D: issue vs updates of multiple inventory items --------------

    @Test
    void issueAndMultipleItemUpdatesLeaveACoherentResult() throws Exception {
        Scenario scenario = scenario(2);
        String token = adminToken(scenario.fixture());

        List<Integer> deactivationStatuses = new ArrayList<>();
        List<HttpResult> results = runTwo(
                () -> issueHttp(scenario, token),
                () -> {
                    deactivationStatuses.add(deactivateItemHttp(
                                    scenario, token, scenario.itemIds().get(0))
                            .status());
                    deactivationStatuses.add(deactivateItemHttp(
                                    scenario, token, scenario.itemIds().get(1))
                            .status());
                    return new HttpResult(200, null);
                });

        HttpResult issue = results.get(0);
        assertThat(deactivationStatuses).containsExactly(200, 200);
        assertThat(itemStatus(scenario.itemIds().get(0))).isEqualTo("INACTIVE");
        assertThat(itemStatus(scenario.itemIds().get(1))).isEqualTo("INACTIVE");
        if (issue.status() == 200) {
            assertThat(orderStatus(scenario.orderId())).isEqualTo("ORDERED");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isEqualTo(1);
        } else {
            assertThat(issue.code()).isEqualTo("INVENTORY_ITEM_INACTIVE");
            assertThat(orderStatus(scenario.orderId())).isEqualTo("DRAFT");
            assertThat(issuedAuditCount(scenario.fixture().orgId())).isZero();
        }
        assertThat(movementCount(scenario.fixture().orgId())).isZero();
        assertThat(balanceCount(scenario.fixture().orgId())).isZero();
    }

    // --- HTTP helpers ----------------------------------------------------------

    private String ordersUrl(Scenario scenario) {
        return BASE + scenario.fixture().orgId() + "/branches/" + scenario.branchId() + "/purchase-orders";
    }

    private HttpResult issueHttp(Scenario scenario, String token) throws Exception {
        MvcResult result = mockMvc.perform(post(ordersUrl(scenario) + "/" + scenario.orderId() + "/issue")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + scenario.expectedVersion() + "}"))
                .andReturn();
        return httpResult(result);
    }

    private HttpResult deactivateSupplierHttp(Scenario scenario, String token) throws Exception {
        MvcResult result = mockMvc.perform(patch(BASE + scenario.fixture().orgId() + "/suppliers/"
                                + scenario.supplierId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andReturn();
        return httpResult(result);
    }

    private HttpResult deactivateItemHttp(Scenario scenario, String token, UUID itemId) throws Exception {
        MvcResult result = mockMvc.perform(patch(BASE + scenario.fixture().orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andReturn();
        return httpResult(result);
    }

    private HttpResult changeBaseUnitHttp(Scenario scenario, String token, UUID itemId, String baseUnit)
            throws Exception {
        MvcResult result = mockMvc.perform(patch(BASE + scenario.fixture().orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"baseUnit\":\"" + baseUnit + "\"}"))
                .andReturn();
        return httpResult(result);
    }

    private HttpResult httpResult(MvcResult result) throws Exception {
        String body = result.getResponse().getContentAsString();
        String code = null;
        if (!body.isBlank()) {
            var node = objectMapper.readTree(body);
            if (node.hasNonNull("code")) {
                code = node.get("code").asText();
            }
        }
        return new HttpResult(result.getResponse().getStatus(), code);
    }

    // --- Deterministic in-transaction probe ------------------------------------

    private Throwable issueWithLockTimeout(Scenario scenario, String lockTimeout) {
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);
        try {
            transaction.executeWithoutResult(status -> {
                setLocalLockTimeout(lockTimeout);
                purchaseOrderService.issue(
                        scenario.fixture().orgId(),
                        scenario.branchId(),
                        scenario.orderId(),
                        scenario.expectedVersion(),
                        scenario.actorId());
            });
            return null;
        } catch (Throwable failure) {
            return failure;
        }
    }

    private void setLocalLockTimeout(String value) {
        entityManager.unwrap(org.hibernate.Session.class).doWork(connection -> {
            try (java.sql.Statement statement = connection.createStatement()) {
                statement.execute("SET LOCAL lock_timeout = '" + value + "'");
            }
        });
    }

    private Throwable deactivateSupplierWithLockTimeout(Scenario scenario, String lockTimeout) {
        TransactionTemplate transaction = new TransactionTemplate(transactionManager);
        try {
            transaction.executeWithoutResult(status -> {
                setLocalLockTimeout(lockTimeout);
                supplierService.update(
                        scenario.fixture().orgId(),
                        scenario.supplierId(),
                        new UpdateSupplierCommand(
                                0, null, null, null, null, null, null, null, SupplierStatus.INACTIVE));
            });
            return null;
        } catch (Throwable failure) {
            return failure;
        }
    }

    private static String sqlStateOf(Throwable failure) {
        Throwable current = failure;
        while (current != null) {
            if (current instanceof SQLException sqlException && sqlException.getSQLState() != null) {
                return sqlException.getSQLState();
            }
            current = current.getCause();
        }
        return null;
    }

    // --- Read helpers ----------------------------------------------------------

    private String orderStatus(UUID orderId) {
        return jdbcTemplate.queryForObject(
                "select status from purchase_orders where id = ?", String.class, orderId);
    }

    private String supplierStatus(UUID supplierId) {
        return jdbcTemplate.queryForObject(
                "select status from suppliers where id = ?", String.class, supplierId);
    }

    private String itemStatus(UUID itemId) {
        return jdbcTemplate.queryForObject(
                "select status from inventory_items where id = ?", String.class, itemId);
    }

    private String itemBaseUnit(UUID itemId) {
        return jdbcTemplate.queryForObject(
                "select base_unit from inventory_items where id = ?", String.class, itemId);
    }

    private long issuedAuditCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ? and action = 'purchase_order.issued'",
                Long.class,
                organizationId);
        return count == null ? 0 : count;
    }

    private long movementCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    private long balanceCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from branch_inventory where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    // --- Concurrency helpers ---------------------------------------------------

    private <T> List<T> runTwo(Callable<T> first, Callable<T> second) throws Exception {
        CountDownLatch ready = new CountDownLatch(2);
        CountDownLatch start = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(2);
        try {
            Future<T> firstResult = pool.submit(() -> {
                ready.countDown();
                start.await();
                return first.call();
            });
            Future<T> secondResult = pool.submit(() -> {
                ready.countDown();
                start.await();
                return second.call();
            });
            assertThat(ready.await(15, TimeUnit.SECONDS)).isTrue();
            start.countDown();
            // Sin timeout, un interbloqueo colgaría la suite; con timeout, falla.
            return List.of(firstResult.get(30, TimeUnit.SECONDS), secondResult.get(30, TimeUnit.SECONDS));
        } finally {
            pool.shutdownNow();
        }
    }

    private static void await(CountDownLatch latch) {
        try {
            if (!latch.await(15, TimeUnit.SECONDS)) {
                throw new IllegalStateException("latch timed out");
            }
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(interrupted);
        }
    }

    /**
     * Espera de forma determinista a que exista una transacción bloqueada esperando
     * un bloqueo de fila sobre {@code relation}. Se usa en lugar de un
     * {@code Thread.sleep} para saber que la emisión ya adquirió sus bloqueos
     * previos (orden y proveedor) y está detenida en el artículo retenido.
     */
    private void awaitBlockingOnRelation(String relation, int timeoutSeconds) throws InterruptedException {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(timeoutSeconds);
        while (System.nanoTime() < deadline) {
            Long waiters = jdbcTemplate.queryForObject(
                    "select count(*) from pg_stat_activity where datname = current_database() "
                            + "and pid <> pg_backend_pid() and wait_event_type = 'Lock' and query like ?",
                    Long.class,
                    "%" + relation + "%");
            if (waiters != null && waiters > 0) {
                return;
            }
            Thread.sleep(25);
        }
        throw new IllegalStateException("no transaction blocked on relation " + relation);
    }

    private static void join(Thread thread) {
        try {
            thread.join(15_000);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(interrupted);
        }
    }
}
