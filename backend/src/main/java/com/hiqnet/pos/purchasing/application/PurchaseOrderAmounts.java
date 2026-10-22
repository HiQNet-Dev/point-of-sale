package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;
import tools.jackson.databind.JsonNode;

/**
 * Frontera decimal exacta de las órdenes de compra.
 *
 * <p>Las cantidades y los costos viajan como <b>string decimal</b> en JSON; nunca
 * se aceptan números en coma flotante. La cantidad reutiliza el contrato exacto
 * de inventario ({@link InventoryQuantity}, {@code NUMERIC(19,6)}) porque la
 * cantidad solicitada se expresa en la unidad base del artículo; el costo
 * unitario usa la escala monetaria del proyecto ({@code NUMERIC(19,4)}).</p>
 */
public final class PurchaseOrderAmounts {

    private static final Pattern LEXICAL = Pattern.compile("-?[0-9]+(?:[.][0-9]+)?");
    private static final BigDecimal MAX_UNIT_COST = new BigDecimal("999999999999999.9999");
    private static final int MONEY_SCALE = 4;

    private PurchaseOrderAmounts() {
    }

    /**
     * Convierte el nodo JSON de cantidad en un decimal positivo exacto.
     *
     * @param node nodo JSON con un string decimal
     * @return cantidad normalizada a escala 6
     * @throws BusinessException si el formato es inválido, excede NUMERIC(19,6) o no es positiva
     */
    public static BigDecimal requireQuantity(JsonNode node) {
        BigDecimal value = InventoryQuantity.parse(node, "quantity");
        if (value.signum() <= 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantity must be greater than zero");
        }
        return value;
    }

    /**
     * Convierte el nodo JSON opcional de costo unitario en un decimal no negativo.
     *
     * @param node nodo JSON con un string decimal, o {@code null} cuando el campo se omite
     * @return costo normalizado a escala 4, o {@code null} si no se informó
     * @throws BusinessException si el formato es inválido, excede NUMERIC(19,4) o es negativo
     */
    public static BigDecimal optionalUnitCost(JsonNode node) {
        if (node == null || node.isNull()) {
            return null;
        }
        if (!node.isTextual()) {
            throw invalidUnitCost("unitCost must be a JSON decimal string");
        }
        String text = node.textValue();
        if (text == null || !LEXICAL.matcher(text).matches()) {
            throw invalidUnitCost("unitCost must be a plain decimal string");
        }
        BigDecimal value = new BigDecimal(text);
        if (value.signum() < 0) {
            throw invalidUnitCost("unitCost must not be negative");
        }
        if (value.scale() > MONEY_SCALE || value.compareTo(MAX_UNIT_COST) > 0) {
            throw invalidUnitCost("unitCost exceeds NUMERIC(19,4)");
        }
        return value.setScale(MONEY_SCALE);
    }

    /**
     * Formatea una cantidad como string decimal de escala 6.
     *
     * @param value cantidad persistida
     * @return representación textual exacta
     */
    public static String formatQuantity(BigDecimal value) {
        return value.setScale(6, RoundingMode.UNNECESSARY).toPlainString();
    }

    /**
     * Formatea un costo unitario como string decimal de escala 4.
     *
     * @param value costo persistido, posiblemente {@code null}
     * @return representación textual exacta, o {@code null}
     */
    public static String formatUnitCost(BigDecimal value) {
        return value == null ? null : value.setScale(MONEY_SCALE, RoundingMode.UNNECESSARY).toPlainString();
    }

    private static BusinessException invalidUnitCost(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, message);
    }
}
