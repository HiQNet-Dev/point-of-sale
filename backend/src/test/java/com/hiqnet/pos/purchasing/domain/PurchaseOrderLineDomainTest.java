package com.hiqnet.pos.purchasing.domain;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class PurchaseOrderLineDomainTest {

    private static final Organization ORGANIZATION =
            Organization.create("Line Test Org", "line-test-org", "UTC", "MXN", "es-MX");
    private static final InventoryItem ITEM =
            InventoryItem.create(ORGANIZATION, "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
    private static final UUID ORDER_ID = UUID.randomUUID();

    private static PurchaseOrderLine line(BigDecimal quantity, BigDecimal unitCost) {
        return PurchaseOrderLine.create(ORGANIZATION.getId(), ORDER_ID, ITEM, 1, quantity, unitCost);
    }

    @Test
    void createCapturesItemSnapshotsAndNormalizesScales() {
        PurchaseOrderLine line = line(new BigDecimal("25000"), new BigDecimal("0.018"));

        assertThat(line.getId()).isInstanceOf(UUID.class);
        assertThat(line.getOrganizationId()).isEqualTo(ORGANIZATION.getId());
        assertThat(line.getPurchaseOrderId()).isEqualTo(ORDER_ID);
        assertThat(line.getInventoryItemId()).isEqualTo(ITEM.getId());
        assertThat(line.getLineNumber()).isEqualTo(1);
        assertThat(line.getItemCodeSnapshot()).isEqualTo("harina-trigo");
        assertThat(line.getItemNameSnapshot()).isEqualTo("Harina de trigo");
        assertThat(line.getUnitSnapshot()).isEqualTo(InventoryBaseUnit.GRAM);
        assertThat(line.getQuantity()).isEqualByComparingTo("25000.000000");
        assertThat(line.getQuantity().scale()).isEqualTo(6);
        assertThat(line.getUnitCost()).isEqualByComparingTo("0.0180");
        assertThat(line.getUnitCost().scale()).isEqualTo(4);
    }

    @Test
    void quantityMustBeAnExactPositiveDecimalWithinNumericBounds() {
        assertThatThrownBy(() -> line(null, null)).isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(BigDecimal.ZERO, null)).isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(new BigDecimal("-1"), null)).isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(new BigDecimal("0.1234567"), null)).isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(new BigDecimal("10000000000000"), null)).isInstanceOf(BusinessException.class);

        assertThat(line(new BigDecimal("0.5"), null).getQuantity()).isEqualByComparingTo("0.500000");
    }

    @Test
    void unitCostIsOptionalAndNeverNegative() {
        assertThat(line(new BigDecimal("1"), null).getUnitCost()).isNull();
        assertThat(line(new BigDecimal("1"), BigDecimal.ZERO).getUnitCost()).isEqualByComparingTo("0.0000");
        assertThatThrownBy(() -> line(new BigDecimal("1"), new BigDecimal("-0.01")))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(new BigDecimal("1"), new BigDecimal("0.12345")))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line(new BigDecimal("1"), new BigDecimal("1000000000000000")))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void updatesRevalidateQuantitiesAndCosts() {
        PurchaseOrderLine line = line(new BigDecimal("10"), null);

        line.updateQuantity(new BigDecimal("12.5"));
        line.updateUnitCost(new BigDecimal("2.50"));

        assertThat(line.getQuantity()).isEqualByComparingTo("12.500000");
        assertThat(line.getUnitCost()).isEqualByComparingTo("2.5000");
        assertThatThrownBy(() -> line.updateQuantity(BigDecimal.ZERO)).isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> line.updateUnitCost(new BigDecimal("-1"))).isInstanceOf(BusinessException.class);
    }

    @Test
    void unitCostCanBeClearedBackToNull() {
        PurchaseOrderLine line = line(new BigDecimal("1"), new BigDecimal("2.50"));
        assertThat(line.getUnitCost()).isEqualByComparingTo("2.5000");

        line.updateUnitCost(null);

        assertThat(line.getUnitCost()).isNull();
    }
}
