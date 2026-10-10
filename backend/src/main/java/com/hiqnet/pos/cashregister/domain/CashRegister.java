package com.hiqnet.pos.cashregister.domain;

import com.hiqnet.pos.branch.domain.Branch;
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
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Branch-owned point of sale inside a branch ({@code Branch -> CashRegister}).
 * Unlike the catalog/modifiers (organization-owned), a cash register belongs to
 * its branch; {@code code} is a normalized slug, unique per branch and
 * immutable (two branches may reuse the same code).
 *
 * <p>No hard delete: a register is retired via status INACTIVE, which removes
 * nothing and never cascades to its branch or any other resource. No shift,
 * sale, balance or hardware state is modeled here.</p>
 */
@Entity
@Table(
        name = "cash_registers",
        uniqueConstraints =
                @UniqueConstraint(
                        name = "cash_registers_branch_code_unique",
                        columnNames = {"branch_id", "code"}))
public class CashRegister {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;

    @Id
    private UUID id;

    // EAGER (default for ManyToOne): every response carries its organization and
    // branch ids; DTO mapping happens outside the service transaction.
    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false)
    private Organization organization;

    @ManyToOne(optional = false)
    @JoinColumn(name = "branch_id", nullable = false)
    private Branch branch;

    @Column(nullable = false, length = MAX_CODE)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private CashRegisterStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected CashRegister() {
    }

    private CashRegister(UUID id, Organization organization, Branch branch, String code, String name) {
        this.id = id;
        this.organization = organization;
        this.branch = branch;
        this.code = code;
        this.name = name;
        this.status = CashRegisterStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Caller must have validated that the branch exists in the
     * organization and that the code is free within the branch. {@code name} is
     * trimmed here; {@code code} is expected already normalized (trimmed,
     * lowercased).
     */
    public static CashRegister create(Organization organization, Branch branch, String code, String name) {
        validateName(name);
        validateCode(code);
        return new CashRegister(UUID.randomUUID(), organization, branch, code, name.trim());
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public Branch getBranch() {
        return branch;
    }

    public String getCode() {
        return code;
    }

    public String getName() {
        return name;
    }

    public CashRegisterStatus getStatus() {
        return status;
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

    public boolean isActive() {
        return status == CashRegisterStatus.ACTIVE;
    }

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName.trim();
    }

    public void updateStatus(CashRegisterStatus newStatus) {
        if (newStatus != null) {
            this.status = newStatus;
        }
    }

    private static void validateName(String value) {
        if (value == null || value.trim().isEmpty()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        if (value.trim().length() > MAX_NAME) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "name must not exceed " + MAX_NAME + " characters");
        }
    }

    private static void validateCode(String value) {
        if (value == null || !SLUG.matcher(value).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CODE,
                    "code must match ^[a-z0-9]+(-[a-z0-9]+)*$");
        }
        if (value.length() > MAX_CODE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CODE,
                    "code must not exceed " + MAX_CODE + " characters");
        }
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
