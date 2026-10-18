package com.hiqnet.pos.purchasing.domain;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/**
 * Línea de una recepción.
 *
 * <p>Referencia la partida original ({@link PurchaseOrderLine}) y el artículo de
 * inventario, y guarda la cantidad recibida en la unidad base congelada en la
 * partida. La cantidad es un decimal exacto estrictamente positivo. La pareja
 * {@code (purchase_receipt_id, purchase_order_line_id)} es única: una recepción
 * no puede recibir dos veces la misma partida.</p>
 */
@Entity
@Table(
        name = "purchase_receipt_lines",
        uniqueConstraints = @UniqueConstraint(
                name = "purchase_receipt_lines_receipt_line_unique",
                columnNames = {"purchase_receipt_id", "purchase_order_line_id"}))
public class PurchaseReceiptLine {

    private static final BigDecimal MAX_QUANTITY = new BigDecimal("9999999999999.999999");
    private static final int QUANTITY_SCALE = 6;

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false, updatable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false, updatable = false)
    private UUID branchId;

    @Column(name = "purchase_receipt_id", nullable = false, updatable = false)
    private UUID purchaseReceiptId;

    @Column(name = "purchase_order_id", nullable = false, updatable = false)
    private UUID purchaseOrderId;

    @Column(name = "purchase_order_line_id", nullable = false, updatable = false)
    private UUID purchaseOrderLineId;

    @Column(name = "inventory_item_id", nullable = false, updatable = false)
    private UUID inventoryItemId;

    @Column(name = "quantity_received", nullable = false, precision = 19, scale = QUANTITY_SCALE)
    private BigDecimal quantityReceived;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    protected PurchaseReceiptLine() {
    }

    private PurchaseReceiptLine(
            UUID organizationId,
            UUID branchId,
            UUID purchaseReceiptId,
            UUID purchaseOrderId,
            UUID purchaseOrderLineId,
            UUID inventoryItemId,
            BigDecimal quantityReceived) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.purchaseReceiptId = purchaseReceiptId;
        this.purchaseOrderId = purchaseOrderId;
        this.purchaseOrderLineId = purchaseOrderLineId;
        this.inventoryItemId = inventoryItemId;
        this.quantityReceived = requireQuantity(quantityReceived);
    }

    /**
     * Crea una línea de recepción.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseReceiptId cabecera de la recepción
     * @param purchaseOrderId orden recibida
     * @param purchaseOrderLineId partida original recibida
     * @param inventoryItemId artículo de la partida
     * @param quantityReceived cantidad recibida en la unidad base de la partida
     * @return línea nueva aún no persistida
     * @throws BusinessException si la cantidad no es un decimal positivo válido
     */
    public static PurchaseReceiptLine create(
            UUID organizationId,
            UUID branchId,
            UUID purchaseReceiptId,
            UUID purchaseOrderId,
            UUID purchaseOrderLineId,
            UUID inventoryItemId,
            BigDecimal quantityReceived) {
        return new PurchaseReceiptLine(
                organizationId,
                branchId,
                purchaseReceiptId,
                purchaseOrderId,
                purchaseOrderLineId,
                inventoryItemId,
                quantityReceived);
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getBranchId() {
        return branchId;
    }

    public UUID getPurchaseReceiptId() {
        return purchaseReceiptId;
    }

    public UUID getPurchaseOrderId() {
        return purchaseOrderId;
    }

    public UUID getPurchaseOrderLineId() {
        return purchaseOrderLineId;
    }

    public UUID getInventoryItemId() {
        return inventoryItemId;
    }

    public BigDecimal getQuantityReceived() {
        return quantityReceived;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    private static BigDecimal requireQuantity(BigDecimal value) {
        if (value == null || value.signum() <= 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantityReceived must be greater than zero");
        }
        if (value.scale() > QUANTITY_SCALE || value.compareTo(MAX_QUANTITY) > 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantityReceived is out of range for NUMERIC(19,6)");
        }
        return value.setScale(QUANTITY_SCALE);
    }

    @PrePersist
    void onCreate() {
        createdAt = Instant.now();
    }
}
