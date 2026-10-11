package com.hiqnet.pos.sales.domain;

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
import jakarta.persistence.Version;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/**
 * Operational aggregate for a transaction under construction. The aggregate's
 * {@link Version} is the concurrency boundary for all line mutations.
 */
@Entity
@Table(name = "sales")
public class Sale {

    private static final int MONEY_SCALE = 4;
    private static final BigDecimal MAX_AMOUNT = new BigDecimal("999999999999999.9999");

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false)
    private UUID branchId;

    @Column(name = "cash_register_id", nullable = false)
    private UUID cashRegisterId;

    @Column(name = "cash_shift_id", nullable = false)
    private UUID cashShiftId;

    @Column(name = "created_by_user_id", nullable = false)
    private UUID createdByUserId;

    @Column(name = "cancelled_by_user_id")
    private UUID cancelledByUserId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private SaleStatus status;

    @Column(name = "total_amount", nullable = false, precision = 19, scale = MONEY_SCALE)
    private BigDecimal totalAmount;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    @Column(name = "cancelled_at")
    private Instant cancelledAt;

    protected Sale() {
    }

    private Sale(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID createdByUserId) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.cashRegisterId = cashRegisterId;
        this.cashShiftId = cashShiftId;
        this.createdByUserId = createdByUserId;
        this.status = SaleStatus.OPEN;
        this.totalAmount = BigDecimal.ZERO.setScale(MONEY_SCALE);
        this.version = 0;
    }

    public static Sale open(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID createdByUserId) {
        if (organizationId == null || branchId == null || cashRegisterId == null || cashShiftId == null || createdByUserId == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "Sale scope and creator are required");
        }
        return new Sale(organizationId, branchId, cashRegisterId, cashShiftId, createdByUserId);
    }

    public void recalculateTotal(BigDecimal total, Instant when) {
        this.totalAmount = normalizeAmount("totalAmount", total);
        this.updatedAt = when;
    }

    public void cancel(UUID actorUserId, Instant when) {
        if (status == SaleStatus.CANCELLED) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SALE_ALREADY_CANCELLED, "The sale is already cancelled");
        }
        if (actorUserId == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "cancelledByUserId is required");
        }
        this.status = SaleStatus.CANCELLED;
        this.cancelledByUserId = actorUserId;
        this.cancelledAt = when;
    }

    public void requireOpen() {
        if (status != SaleStatus.OPEN) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SALE_NOT_OPEN, "The sale is not open");
        }
    }

    private static BigDecimal normalizeAmount(String field, BigDecimal value) {
        if (value == null || value.signum() < 0 || value.compareTo(MAX_AMOUNT) > 0 || value.scale() > MONEY_SCALE) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, field + " is out of range");
        }
        return value.setScale(MONEY_SCALE);
    }

    public UUID getId() { return id; }
    public UUID getOrganizationId() { return organizationId; }
    public UUID getBranchId() { return branchId; }
    public UUID getCashRegisterId() { return cashRegisterId; }
    public UUID getCashShiftId() { return cashShiftId; }
    public UUID getCreatedByUserId() { return createdByUserId; }
    public UUID getCancelledByUserId() { return cancelledByUserId; }
    public SaleStatus getStatus() { return status; }
    public BigDecimal getTotalAmount() { return totalAmount; }
    public long getVersion() { return version; }
    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }
    public Instant getCancelledAt() { return cancelledAt; }

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
