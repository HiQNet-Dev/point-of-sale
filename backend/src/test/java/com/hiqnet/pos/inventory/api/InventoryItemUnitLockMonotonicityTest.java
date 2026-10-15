package com.hiqnet.pos.inventory.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.application.UpdateInventoryItemCommand;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeComponentChange;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeService;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import java.math.BigDecimal;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Intenta falsar la monotonicidad de {@code unit_locked} frente a instancias
 * administradas obsoletas en el Persistence Context, flushing de Hibernate y
 * concurrencia real sobre PostgreSQL.
 *
 * <p>Todas las comprobaciones finales leen el valor directamente de la base de
 * datos por JDBC, sin confiar en el estado de la entidad administrada.</p>
 */
class InventoryItemUnitLockMonotonicityTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired private InventoryItemService items;
    @Autowired private InventoryStockService stock;
    @Autowired private InventoryRecipeService recipes;
    @Autowired private OrganizationRepository organizations;
    @Autowired private BranchRepository branches;
    @Autowired private ProductRepository products;

    private static String suffix() {
        return UUID.randomUUID().toString().substring(0, 8);
    }

    private UUID createUnitItem(RbacFixture fixture) {
        return items.create(
                        fixture.orgId(), "item-" + suffix(), "Item", null, InventoryBaseUnit.UNIT, false)
                .getId();
    }

    private UUID createBranch(RbacFixture fixture) {
        Organization organization = organizations.findById(fixture.orgId()).orElseThrow();
        return branches.saveAndFlush(Branch.create(organization, "Main", "b-" + suffix(), null))
                .getId();
    }

    private String storedUnit(UUID itemId) {
        return jdbcTemplate.queryForObject("select base_unit from inventory_items where id = ?", String.class, itemId);
    }

    private String storedName(UUID itemId) {
        return jdbcTemplate.queryForObject("select name from inventory_items where id = ?", String.class, itemId);
    }

    private boolean storedLocked(UUID itemId) {
        Boolean locked = jdbcTemplate.queryForObject(
                "select unit_locked from inventory_items where id = ?", Boolean.class, itemId);
        return Boolean.TRUE.equals(locked);
    }

    private long storedVersion(UUID itemId) {
        Long version = jdbcTemplate.queryForObject(
                "select version from inventory_items where id = ?", Long.class, itemId);
        return version == null ? -1 : version;
    }

    @Test
    void unitLockPersistsAcrossLaterMetadataAndStatusChanges() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);
        stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "5", null);
        assertThat(storedLocked(itemId)).isTrue();

        mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Renombrado\",\"status\":\"INACTIVE\","
                                + "\"fractionalUnitAllowed\":true}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.unitLocked").value(true))
                .andExpect(jsonPath("$.baseUnit").value("UNIT"))
                .andExpect(jsonPath("$.version").value(1));

        assertThat(storedLocked(itemId)).isTrue();
        assertThat(storedName(itemId)).isEqualTo("Renombrado");
        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
    }

    @Test
    void staleManagedInstanceFlushCannotRevertUnitLock() {
        RbacFixture fixture = fixture();
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);

        new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
            // La entidad entra al Persistence Context con unitLocked=false.
            items.get(fixture.orgId(), itemId);
            // El guard marca unit_locked=true mediante su sentencia dedicada.
            stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "5", null);
            // Se modifica otro campo de la instancia obsoleta y se hace flush.
            items.update(
                    fixture.orgId(),
                    itemId,
                    new UpdateInventoryItemCommand(0, "Renombrado", null, null, null, null));
        });

        assertThat(storedLocked(itemId)).isTrue();
        assertThat(storedName(itemId)).isEqualTo("Renombrado");
        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
    }

    @Test
    void sameTransactionDependencyThenUnitChangeIsRejected() {
        RbacFixture fixture = fixture();
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);

        assertThatThrownBy(() -> new TransactionTemplate(transactionManager).executeWithoutResult(status -> {
                    items.get(fixture.orgId(), itemId);
                    stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "5", null);
                    items.update(
                            fixture.orgId(),
                            itemId,
                            new UpdateInventoryItemCommand(
                                    0, null, null, InventoryBaseUnit.GRAM, null, null));
                }))
                .isInstanceOfSatisfying(BusinessException.class,
                        exception -> assertThat(exception.getCode()).isEqualTo("INVENTORY_ITEM_UNIT_LOCKED"));
    }

    @Test
    void failedDependencyRollsBackUnitLockAndLeavesUnitMutable() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        Organization organization = organizations.findById(fixture.orgId()).orElseThrow();
        UUID itemId = createUnitItem(fixture);
        UUID productId = products.saveAndFlush(Product.create(
                        organization, "SKU-" + suffix().toUpperCase(Locale.ROOT), "Product", BigDecimal.TEN))
                .getId();

        // 0.5 no es válido para UNIT con fractionalUnitAllowed=false: la transacción
        // revierte y el marcador no debe quedar activado.
        assertThatThrownBy(() -> recipes.replace(fixture.orgId(), productId, 0, InventoryRecipeStatus.ACTIVE,
                        List.of(new InventoryRecipeComponentChange(itemId, new BigDecimal("0.5")))))
                .isInstanceOf(BusinessException.class);
        assertThat(storedLocked(itemId)).isFalse();

        mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isOk());
        assertThat(storedUnit(itemId)).isEqualTo("GRAM");
        assertThat(storedLocked(itemId)).isFalse();
    }

    @Test
    void expectedVersionStillEnforcedAfterLock() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);
        stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "5", null);

        mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Primero\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(1));

        mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Segundo\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));

        assertThat(storedName(itemId)).isEqualTo("Primero");
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void rejectedUnitChangeLeavesNoTrace() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);
        stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "5", null);
        long versionBefore = storedVersion(itemId);
        long auditBefore = auditCount(fixture.orgId());

        mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("INVENTORY_ITEM_UNIT_LOCKED"));

        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
        assertThat(storedVersion(itemId)).isEqualTo(versionBefore);
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditBefore);
        assertThat(storedLocked(itemId)).isTrue();
    }

    @Test
    void concurrentMetadataUpdateNeverRevertsUnitLock() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID itemId = createUnitItem(fixture);
        UUID branchId = createBranch(fixture);

        CountDownLatch go = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(2);
        try {
            Future<?> metadataUpdate = pool.submit(() -> {
                go.await(10, TimeUnit.SECONDS);
                mockMvc.perform(patch(BASE + fixture.orgId() + "/inventory-items/" + itemId)
                                .header("Authorization", bearer(token))
                                .contentType(MediaType.APPLICATION_JSON)
                                .content("{\"expectedVersion\":0,\"name\":\"Racer\"}"))
                        .andReturn();
                return null;
            });
            Future<?> dependency = pool.submit(() -> {
                go.await(10, TimeUnit.SECONDS);
                stock.open(fixture.orgId(), branchId, itemId, fixture.userId(), 0, "1", null);
                return null;
            });
            go.countDown();
            metadataUpdate.get(30, TimeUnit.SECONDS);
            dependency.get(30, TimeUnit.SECONDS);
        } finally {
            pool.shutdownNow();
        }

        assertThat(storedLocked(itemId)).isTrue();
        assertThat(storedUnit(itemId)).isEqualTo("UNIT");
    }
}
