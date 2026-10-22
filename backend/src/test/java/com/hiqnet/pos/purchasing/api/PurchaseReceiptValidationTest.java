package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.List;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Validación de cantidades y estados de la recepción sobre PostgreSQL real.
 *
 * <p>Rechaza cantidades cero, negativas y fuera de {@code NUMERIC(19,6)}, partidas
 * duplicadas o ajenas, sucursales o insumos inactivos; confirma que el saldo
 * resultante se calcula desde el balance efectivo (incluso negativo) y que
 * {@code RECEIVED} exige todas las partidas completas, no solo la última recibida.</p>
 */
class PurchaseReceiptValidationTest extends AbstractPurchaseReceiptTest {

    @Test
    void zeroQuantityIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-zero", "10.000000");

        MvcResult result = postReceipt(scope, order.id(), "k-zero", order.version(), order.lineIds().get(0), "0");
        assertThat(result.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(result)).isEqualTo("VALIDATION_ERROR");
        assertThat(countReceipts(scope)).isZero();
    }

    @Test
    void negativeQuantityIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-negative-qty", "10.000000");

        MvcResult result =
                postReceipt(scope, order.id(), "k-negative-qty", order.version(), order.lineIds().get(0), "-1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(result)).isEqualTo("VALIDATION_ERROR");
        assertThat(countReceipts(scope)).isZero();
    }

    @Test
    void excessDecimalPrecisionIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-precision", "10.000000");

        MvcResult result =
                postReceipt(scope, order.id(), "k-precision", order.version(), order.lineIds().get(0), "1.0000001");
        assertThat(result.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(result)).isEqualTo("INVALID_INVENTORY_QUANTITY");
        assertThat(countReceipts(scope)).isZero();
    }

    @Test
    void quantityBeyondNumericRangeIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-range", "10.000000");

        MvcResult result =
                postReceipt(scope, order.id(), "k-range", order.version(), order.lineIds().get(0), "99999999999999");
        assertThat(result.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(result)).isEqualTo("INVALID_INVENTORY_QUANTITY");
        assertThat(countReceipts(scope)).isZero();
    }

    @Test
    void duplicateLineInPayloadIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-duplicate-line", "10.000000");

        MvcResult result = postReceipt(
                scope,
                order.id(),
                "k-duplicate-line",
                order.version(),
                List.of(order.lineIds().get(0), order.lineIds().get(0)),
                List.of("1.000000", "1.000000"));
        assertThat(result.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(result)).isEqualTo("VALIDATION_ERROR");
        assertThat(countReceipts(scope)).isZero();
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isZero();
    }

    @Test
    void lineFromAnotherOrderIsRejected() throws Exception {
        Scope scope = scope();
        Order target = issuedOrder(scope, "po-foreign-target", "10.000000");
        Order other = issuedOrder(scope, "po-foreign-other", "10.000000");

        MvcResult result =
                postReceipt(scope, target.id(), "k-foreign", target.version(), other.lineIds().get(0), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(404);
        assertThat(errorCode(result)).isEqualTo("PURCHASE_ORDER_LINE_NOT_FOUND");
        assertThat(countReceipts(scope)).isZero();
    }

    @Test
    void negativeStartingBalanceRecordsDeltaAndEffectiveResulting() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-negative-balance", "10.000000", "10.000000");
        seedBalance(scope, scope.itemA(), "-3");

        MvcResult result =
                postReceipt(scope, order.id(), "k-negative-balance", order.version(), order.lineIds().get(0), "5.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(201);
        assertThat(receiptStatus(result)).isEqualTo("PARTIALLY_RECEIVED");

        // -3 + 5 = 2, calculado desde el balance efectivo bloqueado, no desde una lectura previa.
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("2.000000");
        assertThat(movementDelta(scope, scope.itemA(), "PURCHASE_RECEIPT")).isEqualByComparingTo("5.000000");
        assertThat(movementResulting(scope, scope.itemA(), "PURCHASE_RECEIPT")).isEqualByComparingTo("2.000000");
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
    }

    @Test
    void receivedStatusRequiresEveryLineComplete() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-all-lines", "4.000000", "6.000000");

        // La partida A queda completa, pero B sigue pendiente: la orden no puede ser RECEIVED.
        MvcResult partial =
                postReceipt(scope, order.id(), "k-all-1", order.version(), order.lineIds().get(0), "4.000000");
        assertThat(partial.getResponse().getStatus()).isEqualTo(201);
        assertThat(receiptStatus(partial)).isEqualTo("PARTIALLY_RECEIVED");
        assertThat(orderStatus(order.id())).isEqualTo("PARTIALLY_RECEIVED");

        MvcResult complete =
                postReceipt(scope, order.id(), "k-all-2", order.version() + 1, order.lineIds().get(1), "6.000000");
        assertThat(complete.getResponse().getStatus()).isEqualTo(201);
        assertThat(receiptStatus(complete)).isEqualTo("RECEIVED");
        assertThat(orderStatus(order.id())).isEqualTo("RECEIVED");
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");
        assertThat(balance(scope, scope.itemB())).isEqualByComparingTo("6.000000");
    }

    @Test
    void cumulativeOverReceiptAcrossTwoReceiptsIsRejected() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-cumulative", "10.000000");

        MvcResult first =
                postReceipt(scope, order.id(), "k-cum-1", order.version(), order.lineIds().get(0), "6.000000");
        assertThat(first.getResponse().getStatus()).isEqualTo(201);

        MvcResult second =
                postReceipt(scope, order.id(), "k-cum-2", order.version() + 1, order.lineIds().get(0), "6.000000");
        assertThat(second.getResponse().getStatus()).isEqualTo(409);
        assertThat(errorCode(second)).isEqualTo("PURCHASE_RECEIPT_EXCEEDS_ORDERED");

        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("6.000000");
        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
    }

    @Test
    void inactiveBranchRejectsReceiptButAllowsRead() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-inactive-branch", "4.000000");

        mockMvc.perform(patch(BASE + scope.fixture().orgId() + "/branches/" + scope.branchId())
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + branchVersion(scope) + ",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        MvcResult result =
                postReceipt(scope, order.id(), "k-inactive-branch", order.version(), order.lineIds().get(0), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(errorCode(result)).isEqualTo("BRANCH_INACTIVE");
        assertThat(countReceipts(scope)).isZero();

        // La lectura de recepciones sigue disponible aunque la sucursal esté inactiva.
        mockMvc.perform(get(receiptStatusUrl(scope, order.id())).header("Authorization", bearer(scope.token())))
                .andExpect(status().isOk());
        mockMvc.perform(get(receiptsUrl(scope, order.id())).header("Authorization", bearer(scope.token())))
                .andExpect(status().isOk());
    }

    @Test
    void inactiveItemRejectsReceipt() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-inactive-item", "4.000000");

        mockMvc.perform(patch(BASE + scope.fixture().orgId() + "/inventory-items/" + scope.itemA())
                        .header("Authorization", bearer(scope.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":" + itemVersion(scope.itemA()) + ",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        MvcResult result =
                postReceipt(scope, order.id(), "k-inactive-item", order.version(), order.lineIds().get(0), "1.000000");
        assertThat(result.getResponse().getStatus()).isEqualTo(409);
        assertThat(errorCode(result)).isEqualTo("INVENTORY_ITEM_INACTIVE");
        assertThat(countReceipts(scope)).isZero();
    }
}
