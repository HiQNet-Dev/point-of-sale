package com.hiqnet.pos.inventory;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

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
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.math.BigDecimal;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;

@ActiveProfiles("test")
@Import(InventoryRecipeAuditRollbackTest.FailingAuditConfig.class)
@SpringBootTest

class InventoryRecipeAuditRollbackTest extends AbstractRbacIntegrationTest {
    @Autowired private OrganizationRepository organizations;
    @Autowired private ProductRepository products;
    @Autowired private ModifierGroupRepository groups;
    @Autowired private ModifierOptionRepository options;
    @Autowired private InventoryItemRepository items;
    @Autowired private InventoryRecipeService recipes;
    @Autowired private ModifierInventoryEffectsService effects;
    @Autowired private JdbcTemplate jdbc;

    @Test
    void failedAuditRollsBackRecipeAndModifierConfigurationRows() {
        RbacFixture fixture = fixture();
        var organization = organizations.findById(fixture.orgId()).orElseThrow();
        String suffix = UUID.randomUUID().toString().substring(0, 8);
        Product product = products.saveAndFlush(Product.create(
                organization, "RECIPE-" + suffix.toUpperCase(Locale.ROOT), "Recipe", BigDecimal.ONE));
        ModifierGroup group = groups.saveAndFlush(ModifierGroup.create(organization, "group-" + suffix, "Group", 0, 1));
        ModifierOption option = options.saveAndFlush(ModifierOption.create(
                organization, group, "option-" + suffix, "Option", BigDecimal.ZERO, 0));
        InventoryItem item = items.saveAndFlush(InventoryItem.create(
                organization, "item-" + suffix, "Item", null, InventoryBaseUnit.GRAM, false));

        assertThatThrownBy(() -> recipes.replace(fixture.orgId(), product.getId(), 0, InventoryRecipeStatus.ACTIVE,
                        List.of(new InventoryRecipeComponentChange(item.getId(), new BigDecimal("18.000000")))))
                .isInstanceOf(AuditException.class);
        assertThat(jdbc.queryForObject("select count(*) from product_inventory_recipes where product_id = ?",
                Long.class, product.getId())).isZero();
        assertThat(jdbc.queryForObject("select count(*) from product_inventory_recipe_components where inventory_item_id = ?",
                Long.class, item.getId())).isZero();

        assertThatThrownBy(() -> effects.replace(fixture.orgId(), option.getId(), 0,
                        List.of(new ModifierInventoryEffectChange(item.getId(), new BigDecimal("-1.000000")))))
                .isInstanceOf(AuditException.class);
        assertThat(jdbc.queryForObject("select count(*) from modifier_option_inventory_configs where modifier_option_id = ?",
                Long.class, option.getId())).isZero();
        assertThat(jdbc.queryForObject("select count(*) from modifier_option_inventory_effects where inventory_item_id = ?",
                Long.class, item.getId())).isZero();
    }

    @TestConfiguration
    static class FailingAuditConfig {
        @Bean
        @Primary
        AuditPort failingAuditPort() {
            return (AuditRecord record) -> {
                throw new AuditException("audit unavailable (forced test failure)");
            };
        }
    }
}
