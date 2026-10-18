package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import org.junit.jupiter.api.Test;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Idempotencia de la recepción de mercancía sobre PostgreSQL real.
 *
 * <p>Cubre la matriz de casos del protocolo: reintento idéntico, líneas en
 * distinto orden, cantidades equivalentes en escala, reintento tras recibir la
 * orden completa (con {@code expectedVersion} obsoleto) y aislamiento de la
 * clave por organización y orden. Cada caso verifica directamente en la base el
 * número de recepciones, líneas, movimientos y eventos de auditoría.</p>
 */
class PurchaseReceiptIdempotencyTest extends AbstractPurchaseReceiptTest {

    @Test
    void sameLinesInDifferentOrderReplayToTheSameReceipt() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-order-insensitive", "4.000000", "6.000000");

        MvcResult first = postReceipt(
                scope, order.id(), "k-order", order.version(), order.lineIds(), List.of("4.000000", "6.000000"));
        assertThat(first.getResponse().getStatus()).isEqualTo(201);
        String firstId = receiptId(first);

        MvcResult replay = postReceipt(
                scope,
                order.id(),
                "k-order",
                order.version(),
                List.of(order.lineIds().get(1), order.lineIds().get(0)),
                List.of("6.000000", "4.000000"));
        assertThat(replay.getResponse().getStatus()).isEqualTo(200);
        assertThat(receiptId(replay)).isEqualTo(firstId);

        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countReceiptLines(scope)).isEqualTo(2);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(2);
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isEqualTo(1);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");
        assertThat(balance(scope, scope.itemB())).isEqualByComparingTo("6.000000");
    }

    @Test
    void decimalScaleVariantsReplayByIdentity() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-scale", "10.000000", "10.000000");

        MvcResult first =
                postReceipt(scope, order.id(), "k-scale", order.version(), order.lineIds(), List.of("1", "1.0"));
        assertThat(first.getResponse().getStatus()).isEqualTo(201);

        MvcResult replayFixedScale = postReceipt(
                scope, order.id(), "k-scale", order.version(), order.lineIds(), List.of("1.000000", "1.000000"));
        assertThat(replayFixedScale.getResponse().getStatus()).isEqualTo(200);

        MvcResult replayShortScale =
                postReceipt(scope, order.id(), "k-scale", order.version(), order.lineIds(), List.of("1", "1"));
        assertThat(replayShortScale.getResponse().getStatus()).isEqualTo(200);

        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(2);
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isEqualTo(1);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("1.000000");
    }

    @Test
    void replayAfterFullyReceivedIgnoresStaleExpectedVersion() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-received-replay", "10.000000", "10.000000");

        MvcResult first = postReceipt(
                scope, order.id(), "k-received", order.version(), order.lineIds(), List.of("10.000000", "10.000000"));
        assertThat(first.getResponse().getStatus()).isEqualTo(201);
        assertThat(receiptStatus(first)).isEqualTo("RECEIVED");
        long receivedVersion = orderVersion(order.id());

        // expectedVersion deliberadamente obsoleto (0): el reintento idéntico no lo exige.
        MvcResult replay =
                postReceipt(scope, order.id(), "k-received", 0, order.lineIds(), List.of("10.000000", "10.000000"));
        assertThat(replay.getResponse().getStatus()).isEqualTo(200);
        assertThat(receiptStatus(replay)).isEqualTo("RECEIVED");
        assertThat(receiptId(replay)).isEqualTo(receiptId(first));

        assertThat(orderVersion(order.id())).isEqualTo(receivedVersion);
        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(2);
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isEqualTo(1);
    }

    @Test
    void sameKeyOnDifferentOrdersIsIndependent() throws Exception {
        Scope scope = scope();
        Order first = issuedOrder(scope, "po-key-order-1", "10.000000");
        Order second = issuedOrder(scope, "po-key-order-2", "10.000000");

        MvcResult a =
                postReceipt(scope, first.id(), "k-shared", first.version(), first.lineIds().get(0), "4.000000");
        MvcResult b =
                postReceipt(scope, second.id(), "k-shared", second.version(), second.lineIds().get(0), "4.000000");

        assertThat(a.getResponse().getStatus()).isEqualTo(201);
        assertThat(b.getResponse().getStatus()).isEqualTo(201);
        assertThat(receiptId(a)).isNotEqualTo(receiptId(b));
        assertThat(countReceipts(scope)).isEqualTo(2);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(2);
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isEqualTo(2);
    }

    @Test
    void keyIsScopedToItsOrganizationAndOrder() throws Exception {
        Scope first = scope();
        Order firstOrder = issuedOrder(first, "po-cross-1", "10.000000");
        Scope second = scope();
        Order secondOrder = issuedOrder(second, "po-cross-2", "10.000000");

        // La misma clave en órdenes de organizaciones distintas es independiente.
        MvcResult a = postReceipt(
                first, firstOrder.id(), "k-scoped", firstOrder.version(), firstOrder.lineIds().get(0), "4.000000");
        MvcResult b = postReceipt(
                second, secondOrder.id(), "k-scoped", secondOrder.version(), secondOrder.lineIds().get(0), "4.000000");
        assertThat(a.getResponse().getStatus()).isEqualTo(201);
        assertThat(b.getResponse().getStatus()).isEqualTo(201);

        // Usar la orden de otra organización desde este ámbito no filtra ni escribe nada.
        MvcResult leak = postReceipt(
                first, secondOrder.id(), "k-leak", secondOrder.version(), secondOrder.lineIds().get(0), "4.000000");
        assertThat(leak.getResponse().getStatus()).isEqualTo(404);
        assertThat(errorCode(leak)).isEqualTo("PURCHASE_ORDER_NOT_FOUND");

        assertThat(countReceipts(first)).isEqualTo(1);
        assertThat(countReceipts(second)).isEqualTo(1);
    }
}
