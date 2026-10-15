package com.hiqnet.pos.purchasing.domain;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class PurchaseOrderDomainTest {

    private static final Organization ORGANIZATION =
            Organization.create("Purchase Order Test Org", "po-test-org", "UTC", "MXN", "es-MX");
    private static final Supplier SUPPLIER =
            Supplier.create(ORGANIZATION, "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
    private static final UUID BRANCH_ID = UUID.randomUUID();
    private static final UUID ACTOR_ID = UUID.randomUUID();

    @Test
    void createStartsAsDraftWithSupplierSnapshotsAndServerCurrency() {
        PurchaseOrder order =
                PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "po-2026-0001", "mxn", ACTOR_ID);

        assertThat(order.getId()).isInstanceOf(UUID.class);
        assertThat(order.getStatus()).isEqualTo(PurchaseOrderStatus.DRAFT);
        assertThat(order.getCurrency()).isEqualTo("MXN");
        assertThat(order.getCode()).isEqualTo("po-2026-0001");
        assertThat(order.getBranchId()).isEqualTo(BRANCH_ID);
        assertThat(order.getCreatedByUserId()).isEqualTo(ACTOR_ID);
        assertThat(order.getSupplierCodeSnapshot()).isEqualTo("proveedor-centro");
        assertThat(order.getSupplierNameSnapshot()).isEqualTo("Proveedor Centro");
        assertThat(order.getVersion()).isZero();
        assertThat(order.getIssuedAt()).isNull();
        assertThat(order.getCancelledAt()).isNull();
    }

    @Test
    void createRejectsInvalidCodeMissingScopeAndBlankCurrency() {
        assertThatThrownBy(() -> PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "PO-1", "MXN", ACTOR_ID))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> PurchaseOrder.create(ORGANIZATION, null, SUPPLIER, "po-1", "MXN", ACTOR_ID))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> PurchaseOrder.create(ORGANIZATION, BRANCH_ID, null, "po-1", "MXN", ACTOR_ID))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "po-1", "MXN", null))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "po-1", "  ", ACTOR_ID))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void draftIsEditableAndSupplierChangeRefreshesSnapshots() {
        PurchaseOrder order =
                PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "po-1", "MXN", ACTOR_ID);
        order.requireDraft();

        Supplier other =
                Supplier.create(ORGANIZATION, "proveedor-norte", "Proveedor Norte", null, null, null, null, null, null);
        order.changeSupplier(other, Instant.parse("2026-10-15T10:00:00Z"));

        assertThat(order.getSupplier()).isSameAs(other);
        assertThat(order.getSupplierCodeSnapshot()).isEqualTo("proveedor-norte");
        assertThat(order.getSupplierNameSnapshot()).isEqualTo("Proveedor Norte");
        assertThat(order.getUpdatedAt()).isEqualTo(Instant.parse("2026-10-15T10:00:00Z"));
    }

    @Test
    void lineMutationTouchesTheAggregateSoVersionCanAdvance() {
        PurchaseOrder order =
                PurchaseOrder.create(ORGANIZATION, BRANCH_ID, SUPPLIER, "po-1", "MXN", ACTOR_ID);
        Instant when = Instant.parse("2026-10-15T11:30:00Z");

        order.registerLineMutation(when);

        assertThat(order.getUpdatedAt()).isEqualTo(when);
    }
}
