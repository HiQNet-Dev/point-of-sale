package com.hiqnet.pos.payment.domain;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/** Immutable payment fact created exclusively by successful atomic checkout. */
@Entity
@Table(name = "payments")
public class Payment {
    private static final int MONEY_SCALE = 4;
    private static final BigDecimal MAX_AMOUNT = new BigDecimal("999999999999999.9999");

    @Id private UUID id;
    @Column(name = "organization_id", nullable = false) private UUID organizationId;
    @Column(name = "branch_id", nullable = false) private UUID branchId;
    @Column(name = "cash_register_id", nullable = false) private UUID cashRegisterId;
    @Column(name = "cash_shift_id", nullable = false) private UUID cashShiftId;
    @Column(name = "sale_id", nullable = false) private UUID saleId;
    @Column(nullable = false) private int position;
    @Enumerated(EnumType.STRING) @Column(nullable = false, length = 16) private PaymentMethod method;
    @Column(nullable = false, precision = 19, scale = MONEY_SCALE) private BigDecimal amount;
    @Column(name = "tendered_amount", precision = 19, scale = MONEY_SCALE) private BigDecimal tenderedAmount;
    @Column(name = "change_amount", nullable = false, precision = 19, scale = MONEY_SCALE) private BigDecimal changeAmount;
    @Column(name = "created_by_user_id", nullable = false) private UUID createdByUserId;
    @Column(name = "created_at", nullable = false, updatable = false) private Instant createdAt;

    protected Payment() {}

    private Payment(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId,
            int position, PaymentMethod method, BigDecimal amount, BigDecimal tenderedAmount, UUID createdByUserId) {
        if (organizationId == null || branchId == null || cashRegisterId == null || cashShiftId == null || saleId == null || createdByUserId == null
                || position < 1 || method == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "Invalid payment scope or method");
        }
        this.id = UUID.randomUUID(); this.organizationId = organizationId; this.branchId = branchId;
        this.cashRegisterId = cashRegisterId; this.cashShiftId = cashShiftId; this.saleId = saleId;
        this.position = position; this.method = method; this.amount = positive("amount", amount); this.createdByUserId = createdByUserId;
        if (method == PaymentMethod.CASH) {
            this.tenderedAmount = positive("tenderedAmount", tenderedAmount);
            if (this.tenderedAmount.compareTo(this.amount) < 0) {
                throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_CASH_TENDERED_AMOUNT, "tenderedAmount must be at least amount");
            }
            this.changeAmount = this.tenderedAmount.subtract(this.amount).setScale(MONEY_SCALE);
        } else {
            if (tenderedAmount != null) {
                throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_PAYMENT, "CARD payments cannot include tenderedAmount");
            }
            this.tenderedAmount = null; this.changeAmount = BigDecimal.ZERO.setScale(MONEY_SCALE);
        }
    }

    public static Payment create(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId,
            int position, PaymentMethod method, BigDecimal amount, BigDecimal tenderedAmount, UUID createdByUserId) {
        return new Payment(organizationId, branchId, cashRegisterId, cashShiftId, saleId, position, method, amount, tenderedAmount, createdByUserId);
    }

    private static BigDecimal positive(String field, BigDecimal value) {
        if (value == null || value.signum() <= 0 || value.scale() > MONEY_SCALE || value.compareTo(MAX_AMOUNT) > 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_PAYMENT, field + " must be positive and fit NUMERIC(19,4)");
        }
        return value.setScale(MONEY_SCALE);
    }

    @PrePersist void onCreate() { createdAt = Instant.now(); }
    public UUID getId() { return id; }
    public UUID getOrganizationId() { return organizationId; }
    public UUID getBranchId() { return branchId; }
    public UUID getCashRegisterId() { return cashRegisterId; }
    public UUID getCashShiftId() { return cashShiftId; }
    public UUID getSaleId() { return saleId; }
    public int getPosition() { return position; }
    public PaymentMethod getMethod() { return method; }
    public BigDecimal getAmount() { return amount; }
    public BigDecimal getTenderedAmount() { return tenderedAmount; }
    public BigDecimal getChangeAmount() { return changeAmount; }
    public UUID getCreatedByUserId() { return createdByUserId; }
    public Instant getCreatedAt() { return createdAt; }
}
