package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;

import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.test.web.servlet.MvcResult;

/**
 * Atomicidad de la recepción: cualquier fallo tardío revierte TODO el hecho.
 *
 * <p>Se inyectan fallos controlados en dos puntos distintos —la auditoría, que
 * ocurre después de escribir cabecera, líneas, balance y movimiento y de
 * transicionar la orden; y la validación de rango de existencias, que ocurre tras
 * persistir cabecera y líneas y bloquear el balance— y se verifica en PostgreSQL
 * que no sobrevive ningún efecto parcial ni se consume la clave de idempotencia,
 * de modo que el mismo reintento idéntico confirma.</p>
 */
@Import(PurchaseReceiptRollbackTest.FailingReceiptAuditConfig.class)
class PurchaseReceiptRollbackTest extends AbstractPurchaseReceiptTest {

    @BeforeEach
    void resetFailureSwitch() {
        FailingReceiptAuditConfig.FAIL_NEXT_RECEIPT_AUDIT.set(false);
    }

    @Test
    void auditFailureRollsBackHeaderLinesStockOrderAndAudit() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-rollback-audit", "10.000000", "10.000000");

        FailingReceiptAuditConfig.FAIL_NEXT_RECEIPT_AUDIT.set(true);
        MvcResult failed = postReceipt(scope, order.id(), "k-rollback", order.version(), order.lineIds().get(0), "4.000000");
        assertThat(failed.getResponse().getStatus()).isEqualTo(500);
        assertThat(errorCode(failed)).isEqualTo("INTERNAL_ERROR");

        // Ningún efecto parcial: ni cabecera, ni líneas, ni balance, ni movimiento, ni transición, ni auditoría.
        assertThat(countReceipts(scope)).isZero();
        assertThat(countReceiptLines(scope)).isZero();
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isZero();
        assertThat(balance(scope, scope.itemA())).isNull();
        assertThat(orderStatus(order.id())).isEqualTo("ORDERED");
        assertThat(orderVersion(order.id())).isEqualTo(order.version());
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isZero();

        // La clave no se consumió: el mismo reintento idéntico confirma.
        MvcResult retry = postReceipt(scope, order.id(), "k-rollback", order.version(), order.lineIds().get(0), "4.000000");
        assertThat(retry.getResponse().getStatus()).isEqualTo(201);
        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countReceiptLines(scope)).isEqualTo(1);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("4.000000");
        assertThat(orderStatus(order.id())).isEqualTo("PARTIALLY_RECEIVED");
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isEqualTo(1);
    }

    @Test
    void lateStockRangeFailureRollsBackHeaderLinesAndBalances() throws Exception {
        Scope scope = scope();
        Order order = issuedOrder(scope, "po-rollback-range", "10.000000");
        // Balance máximo representable: sumarle la recepción desborda NUMERIC(19,6)
        // justo después de persistir cabecera, líneas y de bloquear el balance.
        seedBalance(scope, scope.itemA(), "9999999999999.999999");

        MvcResult failed = postReceipt(scope, order.id(), "k-rollback-range", order.version(), order.lineIds().get(0), "1.000000");
        assertThat(failed.getResponse().getStatus()).isEqualTo(400);
        assertThat(errorCode(failed)).isEqualTo("INVALID_INVENTORY_QUANTITY");

        assertThat(countReceipts(scope)).isZero();
        assertThat(countReceiptLines(scope)).isZero();
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isZero();
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("9999999999999.999999");
        assertThat(orderStatus(order.id())).isEqualTo("ORDERED");
        assertThat(orderVersion(order.id())).isEqualTo(order.version());
        assertThat(auditActionCount(scope, "purchase_receipt.posted")).isZero();

        // Corregido el balance, el mismo reintento idéntico confirma.
        jdbcTemplate.update(
                "update branch_inventory set quantity_on_hand = 0 where branch_id = ? and inventory_item_id = ?",
                scope.branchId(),
                scope.itemA());
        MvcResult retry = postReceipt(scope, order.id(), "k-rollback-range", order.version(), order.lineIds().get(0), "1.000000");
        assertThat(retry.getResponse().getStatus()).isEqualTo(201);
        assertThat(balance(scope, scope.itemA())).isEqualByComparingTo("1.000000");
        assertThat(countReceipts(scope)).isEqualTo(1);
        assertThat(countMovements(scope, "PURCHASE_RECEIPT")).isEqualTo(1);
    }

    /**
     * Reemplaza el puerto de auditoría por uno que delega en el real salvo el
     * primer evento {@code purchase_receipt.posted} tras armar el interruptor, que
     * falla. Permite comprobar la atomicidad sin introducir rutas de fallo en
     * producción.
     */
    @TestConfiguration
    static class FailingReceiptAuditConfig {

        static final AtomicBoolean FAIL_NEXT_RECEIPT_AUDIT = new AtomicBoolean(false);

        @Bean
        @Primary
        AuditPort failingReceiptAuditPort(AuditRecorder delegate) {
            return record -> {
                if ("purchase_receipt.posted".equals(record.action())
                        && FAIL_NEXT_RECEIPT_AUDIT.compareAndSet(true, false)) {
                    throw new AuditException("forced audit failure for atomicity test");
                }
                delegate.record(record);
            };
        }
    }
}
