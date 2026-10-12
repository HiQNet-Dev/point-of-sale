package com.hiqnet.pos.inventory.stock.application;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/** Exact decimal boundary. API quantities are strings; binary floating point is never accepted. */
public final class InventoryQuantity {

    private static final Pattern LEXICAL = Pattern.compile("-?[0-9]+(?:[.][0-9]+)?");
    private static final BigDecimal MAX = new BigDecimal("9999999999999.999999");

    private InventoryQuantity() {}

    public static BigDecimal parse(String text, String field) {
        if (text == null || !LEXICAL.matcher(text).matches()) {
            throw invalid(field + " must be a plain decimal string");
        }
        BigDecimal value = new BigDecimal(text);
        if (value.scale() > 6
                || value.precision() - Math.max(value.scale(), 0) > 13
                || value.abs().compareTo(MAX) > 0) {
            throw invalid(field + " exceeds NUMERIC(19,6)");
        }
        return value.setScale(6, RoundingMode.UNNECESSARY);
    }

    public static void validateRange(BigDecimal value, String field) {
        if (value.abs().compareTo(MAX) > 0) {
            throw invalid(field + " exceeds NUMERIC(19,6)");
        }
    }

    public static String format(BigDecimal value) {
        return value.setScale(6, RoundingMode.UNNECESSARY).toPlainString();
    }

    private static BusinessException invalid(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_INVENTORY_QUANTITY, message);
    }
}
