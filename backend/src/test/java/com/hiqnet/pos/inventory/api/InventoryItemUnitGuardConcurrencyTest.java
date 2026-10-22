package com.hiqnet.pos.inventory.api;

import static org.assertj.core.api.Assertions.assertThat;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.application.UpdateInventoryItemCommand;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectChange;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectsService;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeComponentChange;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeService;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.purchasing.application.CreatePurchaseOrderLineCommand;
import com.hiqnet.pos.purchasing.application.PurchaseOrderDetail;
import com.hiqnet.pos.purchasing.application.PurchaseOrderService;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.shared.error.BusinessException;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * Carreras reales sobre PostgreSQL entre el cambio de unidad base y la creación
 * de dependencias.
 *
 * <p>El artículo se crea {@code UNIT} sin fracción; las dependencias usan la
 * cantidad fraccionaria {@code 0.5}, que solo es válida si la unidad observada
 * fue {@code GRAM}. Así el estado final de la base de datos prueba por sí mismo
 * que ninguna dependencia quedó interpretada con una unidad antigua: no puede
 * existir una dependencia incompatible junto a una unidad cambiada. Las partidas
 * de compra añaden la comprobación directa {@code unit_snapshot == base_unit}.</p>
 */
class InventoryItemUnitGuardConcurrencyTest extends AbstractRbacIntegrationTest {

    @Autowired private OrganizationRepository organizations;
    @Autowired private ProductRepository products;
    @Autowired private ModifierGroupRepository groups;
    @Autowired private ModifierOptionRepository options;
    @Autowired private BranchRepository branches;
    @Autowired private SupplierService suppliers;
    @Autowired private InventoryItemService items;
    @Autowired private InventoryRecipeService recipes;
    @Autowired private ModifierInventoryEffectsService effects;
    @Autowired private InventoryStockService stock;
    @Autowired private PurchaseOrderService purchaseOrders;
    @Autowired private JdbcTemplate jdbc;

    private record Fixture(
            UUID org, UUID item, UUID product, UUID option, UUID branch, UUID supplier, UUID actor) {}

    private Fixture setup() {
        RbacFixture fx = fixture();
        Organization organization = organizations.findById(fx.orgId()).orElseThrow();
        String suffix = UUID.randomUUID().toString().substring(0, 8);
        Product product = products.saveAndFlush(Product.create(
                organization, "SKU-" + suffix.toUpperCase(Locale.ROOT), "Product", BigDecimal.TEN));
        ModifierGroup group = groups.saveAndFlush(ModifierGroup.create(organization, "mg-" + suffix, "Group", 0, 1));
        ModifierOption option = options.saveAndFlush(
                ModifierOption.create(organization, group, "mo-" + suffix, "Option", BigDecimal.ZERO, 0));
        Branch branch = branches.saveAndFlush(Branch.create(organization, "Main", "race-" + suffix, null));
        Supplier supplier = suppliers.create(
                fx.orgId(), "prov-" + suffix, "Prov", null, null, null, null, null, null);
        InventoryItem item = items.create(
                fx.orgId(), "item-" + suffix, "Item", null, InventoryBaseUnit.UNIT, false);
        return new Fixture(
                fx.orgId(), item.getId(), product.getId(), option.getId(), branch.getId(), supplier.getId(), fx.userId());
    }

    @Test
    void unitChangeVersusRecipeCreationLeavesNoIncompatibleDependency() throws Exception {
        Fixture f = setup();
        parallel(
                () -> changeUnit(f),
                () -> recipes.replace(f.org, f.product, 0, InventoryRecipeStatus.ACTIVE,
                        List.of(new InventoryRecipeComponentChange(f.item, new BigDecimal("0.5")))));
        assertCoherent(f, count("select count(*) from product_inventory_recipe_components where inventory_item_id = ?", f.item));
    }

    @Test
    void unitChangeVersusModifierEffectCreationLeavesNoIncompatibleDependency() throws Exception {
        Fixture f = setup();
        parallel(
                () -> changeUnit(f),
                () -> effects.replace(f.org, f.option, 0,
                        List.of(new ModifierInventoryEffectChange(f.item, new BigDecimal("0.5")))));
        assertCoherent(f, count("select count(*) from modifier_option_inventory_effects where inventory_item_id = ?", f.item));
    }

    @Test
    void unitChangeVersusStockMovementLeavesNoIncompatibleDependency() throws Exception {
        Fixture f = setup();
        parallel(
                () -> changeUnit(f),
                () -> stock.adjust(f.org, f.branch, f.item, f.actor, 0, "0.5", "count"));
        assertCoherent(f, count("select count(*) from inventory_movements where inventory_item_id = ?", f.item));
    }

    @Test
    void unitChangeVersusPurchaseOrderLineSnapshotsTheSameUnit() throws Exception {
        Fixture f = setup();
        String suffix = UUID.randomUUID().toString().substring(0, 8);
        PurchaseOrderDetail order = purchaseOrders.create(f.org, f.branch, "po-race-" + suffix, f.supplier, f.actor);
        UUID orderId = order.order().getId();

        parallel(
                () -> changeUnit(f),
                () -> purchaseOrders.addLine(f.org, f.branch, orderId,
                        new CreatePurchaseOrderLineCommand(0, f.item, new BigDecimal("1"), null)));

        String storedUnit = storedUnit(f.item);
        List<String> snapshots = jdbc.queryForList(
                "select unit_snapshot from purchase_order_lines where purchase_order_id = ?", String.class, orderId);
        for (String snapshot : snapshots) {
            assertThat(snapshot).as("line snapshot must match the effective unit").isEqualTo(storedUnit);
        }
        if (!snapshots.isEmpty()) {
            assertThat(storedLocked(f.item)).isTrue();
        }
    }

    /** Cambia la unidad de UNIT a GRAM y devuelve el resultado de negocio. */
    private Object changeUnit(Fixture f) {
        return items.update(
                f.org, f.item, new UpdateInventoryItemCommand(0, null, null, InventoryBaseUnit.GRAM, null, null));
    }

    private void assertCoherent(Fixture f, long dependencyRows) {
        String storedUnit = storedUnit(f.item);
        if (dependencyRows > 0) {
            // Una dependencia de 0.5 solo es aceptable si la unidad observada fue GRAM.
            assertThat(storedUnit).as("dependency created with a stale unit").isEqualTo("GRAM");
            assertThat(storedLocked(f.item)).isTrue();
        }
        assertThat(storedUnit).isIn("UNIT", "GRAM");
    }

    private long count(String sql, UUID itemId) {
        Long value = jdbc.queryForObject(sql, Long.class, itemId);
        return value == null ? 0 : value;
    }

    private String storedUnit(UUID itemId) {
        return jdbc.queryForObject("select base_unit from inventory_items where id = ?", String.class, itemId);
    }

    private boolean storedLocked(UUID itemId) {
        Boolean locked = jdbc.queryForObject(
                "select unit_locked from inventory_items where id = ?", Boolean.class, itemId);
        return Boolean.TRUE.equals(locked);
    }

    @SafeVarargs
    private final List<Outcome> parallel(Callable<?>... actions) throws Exception {
        CountDownLatch ready = new CountDownLatch(actions.length);
        CountDownLatch go = new CountDownLatch(1);
        ExecutorService pool = Executors.newFixedThreadPool(actions.length);
        List<Future<Outcome>> futures = new ArrayList<>();
        try {
            for (Callable<?> action : actions) {
                futures.add(pool.submit(() -> {
                    ready.countDown();
                    if (!go.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("start timeout");
                    try {
                        action.call();
                        return new Outcome(true, null);
                    } catch (BusinessException exception) {
                        return new Outcome(false, exception.getCode());
                    }
                }));
            }
            if (!ready.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("workers not ready");
            go.countDown();
            List<Outcome> outcomes = new ArrayList<>();
            for (Future<Outcome> future : futures) outcomes.add(future.get(30, TimeUnit.SECONDS));
            return outcomes;
        } finally {
            pool.shutdownNow();
        }
    }

    private record Outcome(boolean success, String code) {}
}
