package com.hiqnet.pos.purchasing.domain;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/**
 * Partida de una orden de compra.
 *
 * <p>Referencia un {@link InventoryItem} de la misma organización y congela en el
 * momento de la captura el código, el nombre y la unidad base del artículo
 * (snapshots), de modo que renombrar o reconfigurar el catálogo no reescribe el
 * historial. La cantidad se expresa en la unidad base del artículo y es un
 * decimal exacto estrictamente positivo; el costo unitario pactado es opcional y
 * admite cero.</p>
 *
 * <p>Su ciclo de vida lo gobierna la orden: solo es editable mientras la orden
 * está en {@link PurchaseOrderStatus#DRAFT}. La identidad de la partida es
 * estable porque las recepciones futuras la referenciarán.</p>
 */
@Entity
@Table(
        name = "purchase_order_lines",
        uniqueConstraints = {
            @UniqueConstraint(
                    name = "purchase_order_lines_order_line_unique",
                    columnNames = {"purchase_order_id", "line_number"}),
            @UniqueConstraint(
                    name = "purchase_order_lines_order_item_unique",
                    columnNames = {"purchase_order_id", "inventory_item_id"})
        })
public class PurchaseOrderLine {

    private static final BigDecimal MAX_QUANTITY = new BigDecimal("9999999999999.999999");
    private static final BigDecimal MAX_UNIT_COST = new BigDecimal("999999999999999.9999");
    private static final int QUANTITY_SCALE = 6;
    private static final int MONEY_SCALE = 4;
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false, updatable = false)
    private UUID organizationId;

    @Column(name = "purchase_order_id", nullable = false, updatable = false)
    private UUID purchaseOrderId;

    @Column(name = "inventory_item_id", nullable = false, updatable = false)
    private UUID inventoryItemId;

    @Column(name = "line_number", nullable = false, updatable = false)
    private int lineNumber;

    @Column(name = "item_code_snapshot", nullable = false, length = MAX_CODE, updatable = false)
    private String itemCodeSnapshot;

    @Column(name = "item_name_snapshot", nullable = false, length = MAX_NAME, updatable = false)
    private String itemNameSnapshot;

    @Enumerated(EnumType.STRING)
    @Column(name = "unit_snapshot", nullable = false, length = 16, updatable = false)
    private InventoryBaseUnit unitSnapshot;

    @Column(nullable = false, precision = 19, scale = QUANTITY_SCALE)
    private BigDecimal quantity;

    @Column(name = "unit_cost", precision = 19, scale = MONEY_SCALE)
    private BigDecimal unitCost;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected PurchaseOrderLine() {
    }

    private PurchaseOrderLine(
            UUID organizationId,
            UUID purchaseOrderId,
            InventoryItem item,
            int lineNumber,
            BigDecimal quantity,
            BigDecimal unitCost) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.purchaseOrderId = purchaseOrderId;
        this.inventoryItemId = item.getId();
        this.lineNumber = lineNumber;
        this.itemCodeSnapshot = item.getCode();
        this.itemNameSnapshot = item.getName();
        this.unitSnapshot = item.getBaseUnit();
        this.quantity = requireQuantity(quantity);
        this.unitCost = requireUnitCost(unitCost);
    }

    /**
     * Crea una partida capturando los snapshots del artículo.
     *
     * @param organizationId organización propietaria
     * @param purchaseOrderId orden a la que pertenece
     * @param item artículo de inventario de la misma organización
     * @param lineNumber número de línea asignado por el servidor, único dentro de la orden
     * @param quantity cantidad solicitada en la unidad base del artículo
     * @param unitCost costo unitario pactado opcional; {@code null} cuando no se conoce
     * @return partida nueva aún no persistida
     * @throws BusinessException si la cantidad no es un decimal positivo válido o el costo es negativo
     */
    public static PurchaseOrderLine create(
            UUID organizationId,
            UUID purchaseOrderId,
            InventoryItem item,
            int lineNumber,
            BigDecimal quantity,
            BigDecimal unitCost) {
        if (item == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "inventoryItemId is required");
        }
        return new PurchaseOrderLine(organizationId, purchaseOrderId, item, lineNumber, quantity, unitCost);
    }

    /**
     * Actualiza la cantidad solicitada.
     *
     * @param value nueva cantidad, decimal exacto estrictamente positivo
     */
    public void updateQuantity(BigDecimal value) {
        this.quantity = requireQuantity(value);
    }

    /**
     * Actualiza el costo unitario pactado.
     *
     * @param value nuevo costo unitario, {@code null} para dejarlo sin informar; no admite negativos
     */
    public void updateUnitCost(BigDecimal value) {
        this.unitCost = requireUnitCost(value);
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getPurchaseOrderId() {
        return purchaseOrderId;
    }

    public UUID getInventoryItemId() {
        return inventoryItemId;
    }

    public int getLineNumber() {
        return lineNumber;
    }

    public String getItemCodeSnapshot() {
        return itemCodeSnapshot;
    }

    public String getItemNameSnapshot() {
        return itemNameSnapshot;
    }

    public InventoryBaseUnit getUnitSnapshot() {
        return unitSnapshot;
    }

    public BigDecimal getQuantity() {
        return quantity;
    }

    public BigDecimal getUnitCost() {
        return unitCost;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public Instant getUpdatedAt() {
        return updatedAt;
    }

    private static BigDecimal requireQuantity(BigDecimal value) {
        if (value == null || value.signum() <= 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantity must be greater than zero");
        }
        if (value.scale() > QUANTITY_SCALE || value.compareTo(MAX_QUANTITY) > 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantity is out of range for NUMERIC(19,6)");
        }
        return value.setScale(QUANTITY_SCALE);
    }

    private static BigDecimal requireUnitCost(BigDecimal value) {
        if (value == null) {
            return null;
        }
        if (value.signum() < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "unitCost must not be negative");
        }
        if (value.scale() > MONEY_SCALE || value.compareTo(MAX_UNIT_COST) > 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "unitCost is out of range for NUMERIC(19,4)");
        }
        return value.setScale(MONEY_SCALE);
    }

    @PrePersist
    void onCreate() {
        Instant now = Instant.now();
        createdAt = now;
        updatedAt = now;
    }

    @PreUpdate
    void onUpdate() {
        updatedAt = Instant.now();
    }
}
