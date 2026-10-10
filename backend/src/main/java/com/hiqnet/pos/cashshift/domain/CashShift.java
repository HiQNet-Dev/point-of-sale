package com.hiqnet.pos.cashshift.domain;

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
 * Operational cash session of a branch-owned cash register
 * ({@code CashRegister -> CashShift}).
 *
 * <p>A shift is a HISTORICAL operational record: a register accumulates an
 * unlimited CLOSED history and may hold at most ONE OPEN shift at a time. The
 * one-open rule is enforced by the DATABASE (a partial unique index on
 * {@code cash_register_id WHERE status = 'OPEN'}), never by Java alone — a
 * concurrent double-open is rejected by PostgreSQL.</p>
 *
 * <p>All four references (organization, branch, register, actors) are plain
 * UUIDs: no JPA navigation, every read goes through scoped repositories. The
 * database keeps them tenant-consistent with composite foreign keys, so no
 * cross-organization or cross-branch shift can exist.</p>
 *
 * <p>Money is {@link BigDecimal} (column {@code NUMERIC(19,4)}): never
 * {@code double}/{@code float}. Amounts are operator <i>declarations</i>, not
 * reconciliations — there are no sales, payments or cash movements yet.</p>
 */
@Entity
@Table(name = "cash_shifts")
public class CashShift {

    private static final int MONEY_SCALE = 4;

    /** Upper bound for NUMERIC(19,4): 15 integer digits + 4 decimals. */
    private static final BigDecimal MAX_AMOUNT = new BigDecimal("999999999999999.9999");

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false)
    private UUID branchId;

    @Column(name = "cash_register_id", nullable = false)
    private UUID cashRegisterId;

    @Column(name = "opened_by_user_id", nullable = false)
    private UUID openedByUserId;

    @Column(name = "closed_by_user_id")
    private UUID closedByUserId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private CashShiftStatus status;

    @Column(name = "opening_amount", nullable = false, precision = 19, scale = MONEY_SCALE)
    private BigDecimal openingAmount;

    @Column(name = "closing_declared_amount", precision = 19, scale = MONEY_SCALE)
    private BigDecimal closingDeclaredAmount;

    @Column(name = "opened_at", nullable = false)
    private Instant openedAt;

    @Column(name = "closed_at")
    private Instant closedAt;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected CashShift() {
    }

    private CashShift(
            UUID organizationId,
            UUID branchId,
            UUID cashRegisterId,
            UUID openedByUserId,
            BigDecimal openingAmount,
            Instant openedAt) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.cashRegisterId = cashRegisterId;
        this.openedByUserId = openedByUserId;
        this.status = CashShiftStatus.OPEN;
        this.openingAmount = openingAmount;
        this.openedAt = openedAt;
        this.version = 0;
    }

    /**
     * Opens a shift. The caller must have validated that the register (hence
     * the branch) exists in the organization and is ACTIVE, and that no OPEN
     * shift exists — but the database partial unique index is the final
     * authority on the last point.
     */
    public static CashShift open(
            UUID organizationId,
            UUID branchId,
            UUID cashRegisterId,
            UUID openedByUserId,
            BigDecimal openingAmount,
            Instant openedAt) {
        if (organizationId == null || branchId == null || cashRegisterId == null || openedByUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "organizationId, branchId, cashRegisterId and openedByUserId are required");
        }
        BigDecimal amount = validateAmount("openingAmount", openingAmount);
        return new CashShift(organizationId, branchId, cashRegisterId, openedByUserId, amount, openedAt);
    }

    /**
     * Closes an OPEN shift. Idempotence is deliberately absent: closing an
     * already closed shift is an error (the caller maps it to 409), never a
     * silent no-op, so a closed shift can never be mutated.
     */
    public void close(BigDecimal declaredCashAmount, UUID closedByUserId, Instant closedAt) {
        if (status == CashShiftStatus.CLOSED) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.CASH_SHIFT_ALREADY_CLOSED, "The cash shift is already closed");
        }
        if (closedByUserId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "closedByUserId is required");
        }
        this.closingDeclaredAmount = validateAmount("declaredCashAmount", declaredCashAmount);
        this.closedByUserId = closedByUserId;
        this.closedAt = closedAt;
        this.status = CashShiftStatus.CLOSED;
    }

    /**
     * Money invariant: non-null, {@code >= 0} and at most 4 decimals. An
     * over-precise amount is REJECTED, never silently rounded; valid amounts are
     * normalized to exactly 4 decimals (exact, no information loss).
     */
    private static BigDecimal validateAmount(String field, BigDecimal value) {
        if (value == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, field + " is required");
        }
        if (value.signum() < 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, field + " must be greater than or equal to 0");
        }
        if (value.compareTo(MAX_AMOUNT) > 0) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, field + " exceeds the maximum supported amount");
        }
        if (value.scale() > MONEY_SCALE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    field + " must have at most " + MONEY_SCALE + " decimal places");
        }
        // scale <= 4 here, so setScale is exact (no rounding is possible).
        return value.setScale(MONEY_SCALE);
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

    public UUID getCashRegisterId() {
        return cashRegisterId;
    }

    public UUID getOpenedByUserId() {
        return openedByUserId;
    }

    public UUID getClosedByUserId() {
        return closedByUserId;
    }

    public CashShiftStatus getStatus() {
        return status;
    }

    public BigDecimal getOpeningAmount() {
        return openingAmount;
    }

    public BigDecimal getClosingDeclaredAmount() {
        return closingDeclaredAmount;
    }

    public Instant getOpenedAt() {
        return openedAt;
    }

    public Instant getClosedAt() {
        return closedAt;
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

    public boolean isOpen() {
        return status == CashShiftStatus.OPEN;
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
