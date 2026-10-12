package com.hiqnet.pos.inventory.stock;
import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.shared.error.BusinessException;
import org.junit.jupiter.api.Test;
class InventoryQuantityTest {
    @Test void parsesExactRangeAndFormatsCanonicalSixPlaces() {
        assertThat(InventoryQuantity.format(InventoryQuantity.parse("9999999999999.999999","quantity"))).isEqualTo("9999999999999.999999");
        assertThat(InventoryQuantity.format(InventoryQuantity.parse("0.000001","quantity"))).isEqualTo("0.000001");
        assertThat(InventoryQuantity.format(InventoryQuantity.parse("-2.5","quantity"))).isEqualTo("-2.500000");
    }
    @Test void rejectsNonStringSyntaxExponentsAndScaleOrPrecisionOverflow() {
        for(String bad:new String[]{""," ","1e3","1,25","NaN","Infinity","0.0000001","10000000000000"})
            assertThatThrownBy(()->InventoryQuantity.parse(bad,"quantity")).isInstanceOf(BusinessException.class);
    }
    @Test void rejectsResultAboveNumericRangeWithoutRounding() {
        assertThatThrownBy(()->InventoryQuantity.validateRange(new java.math.BigDecimal("10000000000000.000000"),"result"))
                .isInstanceOf(BusinessException.class);
    }
}
