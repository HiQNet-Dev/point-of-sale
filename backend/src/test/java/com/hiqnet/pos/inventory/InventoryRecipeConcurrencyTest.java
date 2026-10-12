package com.hiqnet.pos.inventory;

import static org.assertj.core.api.Assertions.assertThat;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectChange;
import com.hiqnet.pos.inventory.modifier.application.ModifierInventoryEffectsService;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeComponentChange;
import com.hiqnet.pos.inventory.recipe.application.InventoryRecipeService;
import com.hiqnet.pos.inventory.recipe.domain.InventoryRecipeStatus;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

class InventoryRecipeConcurrencyTest extends AbstractRbacIntegrationTest {
    @Autowired private OrganizationRepository organizations;
    @Autowired private ProductRepository products;
    @Autowired private ModifierGroupRepository groups;
    @Autowired private ModifierOptionRepository options;
    @Autowired private InventoryItemRepository items;
    @Autowired private InventoryRecipeService recipes;
    @Autowired private ModifierInventoryEffectsService effects;

    @Test
    void onlyOneConcurrentRecipeReplaceWinsForTheSameExpectedVersion() throws Exception {
        var f = setup();
        assertOneWinner(parallel(
                () -> recipes.replace(f.org, f.product, 0, InventoryRecipeStatus.ACTIVE,
                        List.of(new InventoryRecipeComponentChange(f.item, new BigDecimal("1")))),
                () -> recipes.replace(f.org, f.product, 0, InventoryRecipeStatus.ACTIVE,
                        List.of(new InventoryRecipeComponentChange(f.item, new BigDecimal("2"))))));
    }

    @Test
    void onlyOneConcurrentModifierEffectsReplaceWinsForTheSameExpectedVersion() throws Exception {
        var f = setup();
        assertOneWinner(parallel(
                () -> effects.replace(f.org, f.option, 0,
                        List.of(new ModifierInventoryEffectChange(f.item, new BigDecimal("1")))),
                () -> effects.replace(f.org, f.option, 0,
                        List.of(new ModifierInventoryEffectChange(f.item, new BigDecimal("2"))))));
    }

    private Fixture setup() {
        var fx = fixture();
        var organization = organizations.findById(fx.orgId()).orElseThrow();
        String suffix = UUID.randomUUID().toString().substring(0, 8);
        Product product = products.saveAndFlush(Product.create(organization, "RACE-" + suffix, "Recipe", BigDecimal.ONE));
        ModifierGroup group = groups.saveAndFlush(ModifierGroup.create(organization, "race-" + suffix, "Group", 0, 1));
        ModifierOption option = options.saveAndFlush(ModifierOption.create(
                organization, group, "race-" + suffix, "Option", BigDecimal.ZERO, 0));
        InventoryItem item = items.saveAndFlush(InventoryItem.create(
                organization, "race-" + suffix, "Item", null, InventoryBaseUnit.GRAM, false));
        return new Fixture(fx.orgId(), product.getId(), option.getId(), item.getId());
    }

    private static void assertOneWinner(List<Outcome> outcomes) {
        assertThat(outcomes.stream().filter(Outcome::success).count()).isEqualTo(1);
        assertThat(outcomes.stream().filter(o -> !o.success && "CONCURRENT_MODIFICATION".equals(o.code)).count())
                .isEqualTo(1);
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

    private record Fixture(UUID org, UUID product, UUID option, UUID item) {}
    private record Outcome(boolean success, String code) {}
}
