package com.hiqnet.pos.purchasing.domain;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.JoinColumn;
import jakarta.persistence.ManyToOne;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import jakarta.persistence.Version;
import java.time.Instant;
import java.util.Locale;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Orden de compra propiedad de una organización y dirigida a una de sus
 * sucursales.
 *
 * <p>Es la raíz del agregado: cabecera y partidas comparten el contador
 * {@link #version}, de modo que cualquier mutación de una partida también
 * incrementa la versión de la cabecera. El código comercial es un slug inmutable
 * y único por organización; la moneda es un snapshot de la organización que el
 * cliente no puede fijar ni modificar; y el proveedor se guarda junto a sus
 * snapshots de código y nombre para preservar el historial aunque el catálogo
 * cambie.</p>
 *
 * <p>El ciclo de vida lo gobierna el propio agregado: una orden nace en
 * {@link PurchaseOrderStatus#DRAFT} y solo puede emitirse ({@link #issue}) desde
 * DRAFT, recibir mercancía ({@link #applyReceipt}) desde ORDERED o
 * PARTIALLY_RECEIVED, o cancelarse ({@link #cancel}) desde DRAFT u ORDERED. La
 * cancelación es irreversible y conserva los datos históricos de emisión cuando
 * la orden ya estaba emitida. La orden nunca genera movimientos de inventario por
 * sí misma: los movimientos los produce la recepción de mercancía.</p>
 */
@Entity
@Table(
        name = "purchase_orders",
        uniqueConstraints = @UniqueConstraint(
                name = "purchase_orders_organization_code_unique",
                columnNames = {"organization_id", "code"}))
public class PurchaseOrder {

    private static final Pattern CODE = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_CODE = 64;
    private static final int CODE_LENGTH = 3;

    @Id
    private UUID id;

    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false, updatable = false)
    private Organization organization;

    @Column(name = "branch_id", nullable = false, updatable = false)
    private UUID branchId;

    @ManyToOne(optional = false)
    @JoinColumn(name = "supplier_id", nullable = false)
    private Supplier supplier;

    @Column(nullable = false, length = MAX_CODE, updatable = false)
    private String code;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 24)
    private PurchaseOrderStatus status;

    @Column(nullable = false, length = 3, updatable = false)
    private String currency;

    @Column(name = "supplier_code_snapshot", nullable = false, length = 64)
    private String supplierCodeSnapshot;

    @Column(name = "supplier_name_snapshot", nullable = false, length = 120)
    private String supplierNameSnapshot;

    @Column(name = "created_by_user_id", nullable = false, updatable = false)
    private UUID createdByUserId;

    @Column(name = "issued_by_user_id")
    private UUID issuedByUserId;

    @Column(name = "cancelled_by_user_id")
    private UUID cancelledByUserId;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    @Column(name = "issued_at")
    private Instant issuedAt;

    @Column(name = "cancelled_at")
    private Instant cancelledAt;

    protected PurchaseOrder() {
    }

    private PurchaseOrder(
            Organization organization,
            UUID branchId,
            Supplier supplier,
            String code,
            String currency,
            UUID createdByUserId) {
        this.id = UUID.randomUUID();
        this.organization = organization;
        this.branchId = branchId;
        this.supplier = supplier;
        this.code = code;
        this.status = PurchaseOrderStatus.DRAFT;
        this.currency = currency;
        this.supplierCodeSnapshot = supplier.getCode();
        this.supplierNameSnapshot = supplier.getName();
        this.createdByUserId = createdByUserId;
        this.version = 0;
    }

    /**
     * Crea una orden en borrador con la moneda y los snapshots del proveedor.
     *
     * @param organization organización propietaria
     * @param branchId sucursal de destino, dentro de la misma organización
     * @param supplier proveedor de la organización; sus datos se capturan como snapshot
     * @param code código comercial normalizado (slug en minúsculas)
     * @param currency moneda ISO 4217 copiada de la organización, nunca del cliente
     * @param createdByUserId actor autenticado que crea la orden
     * @return orden nueva en estado DRAFT, aún no persistida
     * @throws BusinessException si falta un dato obligatorio o el código no es un slug válido
     */
    public static PurchaseOrder create(
            Organization organization,
            UUID branchId,
            Supplier supplier,
            String code,
            String currency,
            UUID createdByUserId) {
        if (organization == null) {
            throw new IllegalArgumentException("organization is required");
        }
        if (branchId == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "branchId is required");
        }
        if (supplier == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "supplierId is required");
        }
        if (createdByUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "createdByUserId is required");
        }
        return new PurchaseOrder(
                organization, branchId, supplier, requireCode(code), requireCurrency(currency), createdByUserId);
    }

    /**
     * Reasigna el proveedor de un borrador y re-captura sus snapshots.
     *
     * <p>Solo es válido en {@link PurchaseOrderStatus#DRAFT}. Si el proveedor es
     * el mismo que ya tenía la orden, no se produce ningún cambio real y el
     * agregado permanece limpio (la versión no se incrementa).</p>
     *
     * @param newSupplier nuevo proveedor, de la misma organización
     * @param when instante del cambio, fijado por el servidor
     * @throws BusinessException si la orden no está en DRAFT o falta el proveedor
     */
    public void changeSupplier(Supplier newSupplier, Instant when) {
        requireDraft();
        if (newSupplier == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "supplierId is required");
        }
        this.supplier = newSupplier;
        this.supplierCodeSnapshot = newSupplier.getCode();
        this.supplierNameSnapshot = newSupplier.getName();
        this.updatedAt = when;
    }

    /**
     * Marca el agregado como modificado tras una mutación de sus partidas.
     *
     * <p>Las partidas viven en su propia tabla, por lo que insertar, actualizar o
     * eliminar una línea no marca la cabecera por sí solo. Este método toca
     * {@code updatedAt} para que Hibernate detecte el agregado como sucio y
     * incremente {@link #version} al hacer flush: la versión de la cabecera es el
     * punto de sincronización optimista de toda mutación de partidas.</p>
     *
     * @param when instante del cambio, fijado por el servidor
     */
    public void registerLineMutation(Instant when) {
        this.updatedAt = when;
    }

    /**
     * Exige que la orden esté en borrador antes de editar cabecera o partidas.
     *
     * @throws BusinessException con {@code 409 PURCHASE_ORDER_NOT_DRAFT} si la orden no está en DRAFT
     */
    public void requireDraft() {
        if (status != PurchaseOrderStatus.DRAFT) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_NOT_DRAFT,
                    "The purchase order is not editable because it is not in DRAFT");
        }
    }

    /**
     * Emite la orden: transición irreversible {@code DRAFT → ORDERED}.
     *
     * <p>Registra el actor y el instante de emisión, ambos fijados por el
     * servidor. Emitir <b>no</b> modifica existencias: es un registro interno de
     * que la orden fue cursada; no implica envío de correos ni documentos al
     * proveedor. Una orden ya emitida o cancelada no puede volver a emitirse.</p>
     *
     * @param actorUserId usuario autenticado que emite la orden
     * @param when instante de la emisión, fijado por el servidor
     * @throws BusinessException con {@code 409 PURCHASE_ORDER_NOT_DRAFT} si la orden no está en DRAFT
     */
    public void issue(UUID actorUserId, Instant when) {
        if (status != PurchaseOrderStatus.DRAFT) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_NOT_DRAFT,
                    "The purchase order can only be issued from DRAFT");
        }
        if (actorUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "issuedByUserId is required");
        }
        this.status = PurchaseOrderStatus.ORDERED;
        this.issuedByUserId = actorUserId;
        this.issuedAt = when;
        this.updatedAt = when;
    }

    /**
     * Cancela la orden: transición irreversible {@code DRAFT|ORDERED → CANCELLED}.
     *
     * <p>Registra el actor y el instante de cancelación, ambos fijados por el
     * servidor. Si la orden ya estaba emitida, conserva sus datos históricos de
     * emisión ({@code issued_at} e {@code issued_by_user_id}); si era un borrador,
     * esos campos permanecen {@code NULL}. Cancelar <b>no</b> modifica existencias.
     * Una orden cancelada no puede volver a emitirse ni cancelarse.</p>
     *
     * @param actorUserId usuario autenticado que cancela la orden
     * @param when instante de la cancelación, fijado por el servidor
     * @throws BusinessException con {@code 409 PURCHASE_ORDER_ALREADY_CANCELLED} si ya estaba cancelada
     */
    public void cancel(UUID actorUserId, Instant when) {
        if (status == PurchaseOrderStatus.CANCELLED) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_ALREADY_CANCELLED,
                    "The purchase order is already cancelled");
        }
        if (actorUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "cancelledByUserId is required");
        }
        this.status = PurchaseOrderStatus.CANCELLED;
        this.cancelledByUserId = actorUserId;
        this.cancelledAt = when;
        this.updatedAt = when;
    }

    /**
     * Aplica el resultado de una recepción confirmada: {@code ORDERED →
     * PARTIALLY_RECEIVED} o {@code → RECEIVED}.
     *
     * <p>Solo es válido desde {@link PurchaseOrderStatus#ORDERED} o
     * {@link PurchaseOrderStatus#PARTIALLY_RECEIVED}; una orden cancelada o ya
     * completamente recibida no admite más mercancía. Conserva intactos los datos
     * históricos de emisión y toca {@code updatedAt} para que {@code @Version}
     * incremente la versión del agregado.</p>
     *
     * @param newStatus estado resultante tras la recepción
     * @param when instante del cambio, fijado por el servidor
     * @throws BusinessException si la orden no admite recepciones o el estado es inválido
     */
    public void applyReceipt(PurchaseOrderStatus newStatus, Instant when) {
        if (newStatus != PurchaseOrderStatus.PARTIALLY_RECEIVED && newStatus != PurchaseOrderStatus.RECEIVED) {
            throw new IllegalArgumentException("applyReceipt status must be PARTIALLY_RECEIVED or RECEIVED");
        }
        if (!status.isReceivable()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_NOT_RECEIVABLE,
                    "The purchase order cannot receive goods in its current status");
        }
        this.status = newStatus;
        this.updatedAt = when;
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public UUID getBranchId() {
        return branchId;
    }

    public Supplier getSupplier() {
        return supplier;
    }

    public String getCode() {
        return code;
    }

    public PurchaseOrderStatus getStatus() {
        return status;
    }

    public String getCurrency() {
        return currency;
    }

    public String getSupplierCodeSnapshot() {
        return supplierCodeSnapshot;
    }

    public String getSupplierNameSnapshot() {
        return supplierNameSnapshot;
    }

    public UUID getCreatedByUserId() {
        return createdByUserId;
    }

    public UUID getIssuedByUserId() {
        return issuedByUserId;
    }

    public UUID getCancelledByUserId() {
        return cancelledByUserId;
    }

    public long getVersion() {
        return version;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public Instant getUpdatedAt() {
        return updatedAt;
    }

    public Instant getIssuedAt() {
        return issuedAt;
    }

    public Instant getCancelledAt() {
        return cancelledAt;
    }

    private static String requireCode(String value) {
        if (value == null || value.length() > MAX_CODE || !CODE.matcher(value).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CODE,
                    "code must match ^[a-z0-9]+(-[a-z0-9]+)*$ and not exceed " + MAX_CODE + " characters");
        }
        return value;
    }

    private static String requireCurrency(String value) {
        if (value == null || value.isBlank() || value.trim().length() != CODE_LENGTH) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "currency is required");
        }
        return value.trim().toUpperCase(Locale.ROOT);
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
