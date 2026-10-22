package com.hiqnet.pos.purchasing.domain;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/**
 * Recepción confirmada e inmutable de una orden de compra emitida.
 *
 * <p>Una orden puede tener múltiples recepciones; cada una registra el actor, el
 * instante y la clave de idempotencia que la identifica dentro de la pareja
 * organización-orden. El {@code payloadFingerprint} (SHA-256 canónico de las
 * líneas) permite distinguir un reintento idéntico (devuelve el resultado
 * original) de una reutilización distinta de la clave (conflicto). Una vez
 * persistida no se modifica: solo se leen cabecera y líneas.</p>
 */
@Entity
@Table(
        name = "purchase_receipts",
        uniqueConstraints = @UniqueConstraint(
                name = "purchase_receipts_idempotency_unique",
                columnNames = {"organization_id", "purchase_order_id", "idempotency_key"}))
public class PurchaseReceipt {

    private static final int MAX_KEY = 128;

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false, updatable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false, updatable = false)
    private UUID branchId;

    @Column(name = "purchase_order_id", nullable = false, updatable = false)
    private UUID purchaseOrderId;

    @Column(name = "idempotency_key", nullable = false, length = MAX_KEY, updatable = false)
    private String idempotencyKey;

    @Column(name = "payload_fingerprint", nullable = false, length = 64, updatable = false)
    private String payloadFingerprint;

    @Column(name = "received_by_user_id", nullable = false, updatable = false)
    private UUID receivedByUserId;

    @Column(name = "received_at", nullable = false, updatable = false)
    private Instant receivedAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    protected PurchaseReceipt() {
    }

    private PurchaseReceipt(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            String idempotencyKey,
            String payloadFingerprint,
            UUID receivedByUserId,
            Instant receivedAt) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.purchaseOrderId = purchaseOrderId;
        this.idempotencyKey = idempotencyKey;
        this.payloadFingerprint = payloadFingerprint;
        this.receivedByUserId = receivedByUserId;
        this.receivedAt = receivedAt;
    }

    /**
     * Crea una recepción inmutable ya validada por el servicio.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino
     * @param purchaseOrderId orden recibida
     * @param idempotencyKey clave de idempotencia normalizada
     * @param payloadFingerprint huella canónica de las líneas
     * @param receivedByUserId actor autenticado que confirma la recepción
     * @param receivedAt instante de la recepción, fijado por el servidor
     * @return recepción nueva aún no persistida
     * @throws BusinessException si falta un dato obligatorio
     */
    public static PurchaseReceipt create(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            String idempotencyKey,
            String payloadFingerprint,
            UUID receivedByUserId,
            Instant receivedAt) {
        if (organizationId == null || branchId == null || purchaseOrderId == null || receivedByUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "receipt scope is required");
        }
        if (idempotencyKey == null || idempotencyKey.isBlank() || idempotencyKey.length() > MAX_KEY) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "idempotencyKey is required");
        }
        if (payloadFingerprint == null || payloadFingerprint.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "payloadFingerprint is required");
        }
        return new PurchaseReceipt(
                organizationId,
                branchId,
                purchaseOrderId,
                idempotencyKey,
                payloadFingerprint,
                receivedByUserId,
                receivedAt);
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

    public UUID getPurchaseOrderId() {
        return purchaseOrderId;
    }

    public String getIdempotencyKey() {
        return idempotencyKey;
    }

    public String getPayloadFingerprint() {
        return payloadFingerprint;
    }

    public UUID getReceivedByUserId() {
        return receivedByUserId;
    }

    public Instant getReceivedAt() {
        return receivedAt;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    @PrePersist
    void onCreate() {
        createdAt = Instant.now();
    }
}
