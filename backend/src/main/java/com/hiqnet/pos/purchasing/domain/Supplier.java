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
 * Proveedor propiedad de una organización.
 *
 * <p>El código comercial se normaliza como slug, es inmutable y único dentro de
 * la organización. Los datos de contacto son información maestra editable; la
 * baja se representa con {@link SupplierStatus#INACTIVE} para no perder futuras
 * referencias históricas de Purchasing.</p>
 */
@Entity
@Table(
        name = "suppliers",
        uniqueConstraints = @UniqueConstraint(
                name = "suppliers_organization_code_unique",
                columnNames = {"organization_id", "code"}))
public class Supplier {

    private static final Pattern CODE = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final Pattern EMAIL_FORMAT = Pattern.compile("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$");
    private static final int MAX_CODE = 64;
    private static final int MAX_NAME = 120;
    private static final int MAX_LEGAL_NAME = 160;
    private static final int MAX_TAX_ID = 64;
    private static final int MAX_CONTACT_NAME = 120;
    private static final int MAX_EMAIL = 254;
    private static final int MAX_PHONE = 40;
    private static final int MAX_ADDRESS = 500;

    @Id
    private UUID id;

    @ManyToOne(optional = false)
    @JoinColumn(name = "organization_id", nullable = false, updatable = false)
    private Organization organization;

    @Column(nullable = false, length = MAX_CODE, updatable = false)
    private String code;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(name = "legal_name", length = MAX_LEGAL_NAME)
    private String legalName;

    @Column(name = "tax_id", length = MAX_TAX_ID)
    private String taxId;

    @Column(name = "contact_name", length = MAX_CONTACT_NAME)
    private String contactName;

    @Column(length = MAX_EMAIL)
    private String email;

    @Column(length = MAX_PHONE)
    private String phone;

    @Column(length = MAX_ADDRESS)
    private String address;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private SupplierStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Supplier() {
    }

    private Supplier(
            Organization organization,
            String code,
            String name,
            String legalName,
            String taxId,
            String contactName,
            String email,
            String phone,
            String address) {
        this.id = UUID.randomUUID();
        this.organization = organization;
        this.code = requireCode(code);
        this.name = requireName(name);
        this.legalName = normalizeOptional(legalName, "legalName", MAX_LEGAL_NAME);
        this.taxId = normalizeOptional(taxId, "taxId", MAX_TAX_ID);
        this.contactName = normalizeOptional(contactName, "contactName", MAX_CONTACT_NAME);
        this.email = normalizeEmail(email);
        this.phone = normalizeOptional(phone, "phone", MAX_PHONE);
        this.address = normalizeOptional(address, "address", MAX_ADDRESS);
        this.status = SupplierStatus.ACTIVE;
    }

    /**
     * Crea un proveedor activo con un código comercial previamente normalizado.
     *
     * @param organization organización propietaria
     * @param code código comercial slug, único en la organización
     * @param name nombre comercial obligatorio
     * @param legalName razón social opcional
     * @param taxId identificador fiscal opcional
     * @param contactName contacto principal opcional
     * @param email correo de contacto opcional
     * @param phone teléfono de contacto opcional
     * @param address dirección opcional
     * @return proveedor nuevo aún no persistido
     */
    public static Supplier create(
            Organization organization,
            String code,
            String name,
            String legalName,
            String taxId,
            String contactName,
            String email,
            String phone,
            String address) {
        if (organization == null) {
            throw new IllegalArgumentException("organization is required");
        }
        return new Supplier(organization, code, name, legalName, taxId, contactName, email, phone, address);
    }

    public UUID getId() {
        return id;
    }

    public Organization getOrganization() {
        return organization;
    }

    public String getCode() {
        return code;
    }

    public String getName() {
        return name;
    }

    public String getLegalName() {
        return legalName;
    }

    public String getTaxId() {
        return taxId;
    }

    public String getContactName() {
        return contactName;
    }

    public String getEmail() {
        return email;
    }

    public String getPhone() {
        return phone;
    }

    public String getAddress() {
        return address;
    }

    public SupplierStatus getStatus() {
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

    /**
     * Actualiza el nombre comercial obligatorio.
     *
     * @param value nuevo nombre comercial
     */
    public void updateName(String value) {
        this.name = requireName(value);
    }

    /**
     * Actualiza la razón social; una cadena en blanco elimina el valor.
     *
     * @param value nueva razón social opcional
     */
    public void updateLegalName(String value) {
        this.legalName = normalizeOptional(value, "legalName", MAX_LEGAL_NAME);
    }

    /**
     * Actualiza el identificador fiscal; una cadena en blanco elimina el valor.
     *
     * @param value nuevo identificador fiscal opcional
     */
    public void updateTaxId(String value) {
        this.taxId = normalizeOptional(value, "taxId", MAX_TAX_ID);
    }

    /**
     * Actualiza el contacto principal; una cadena en blanco elimina el valor.
     *
     * @param value nuevo contacto opcional
     */
    public void updateContactName(String value) {
        this.contactName = normalizeOptional(value, "contactName", MAX_CONTACT_NAME);
    }

    /**
     * Actualiza el correo de contacto; una cadena en blanco elimina el valor.
     *
     * @param value nuevo correo opcional
     */
    public void updateEmail(String value) {
        this.email = normalizeEmail(value);
    }

    /**
     * Actualiza el teléfono de contacto; una cadena en blanco elimina el valor.
     *
     * @param value nuevo teléfono opcional
     */
    public void updatePhone(String value) {
        this.phone = normalizeOptional(value, "phone", MAX_PHONE);
    }

    /**
     * Actualiza la dirección; una cadena en blanco elimina el valor.
     *
     * @param value nueva dirección opcional
     */
    public void updateAddress(String value) {
        this.address = normalizeOptional(value, "address", MAX_ADDRESS);
    }

    /**
     * Cambia el estado operativo sin eliminar el proveedor.
     *
     * @param value nuevo estado
     */
    public void updateStatus(SupplierStatus value) {
        if (value == null) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "status is required");
        }
        this.status = value;
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

    private static String requireName(String value) {
        if (value == null || value.trim().isEmpty()) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name is required");
        }
        String normalized = value.trim();
        if (normalized.length() > MAX_NAME) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "name must not exceed " + MAX_NAME + " characters");
        }
        return normalized;
    }

    private static String normalizeOptional(String value, String field, int maxLength) {
        if (value == null || value.trim().isEmpty()) {
            return null;
        }
        String normalized = value.trim();
        if (normalized.length() > maxLength) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    field + " must not exceed " + maxLength + " characters");
        }
        return normalized;
    }

    private static String normalizeEmail(String value) {
        String normalized = normalizeOptional(value, "email", MAX_EMAIL);
        if (normalized == null) {
            return null;
        }
        normalized = normalized.toLowerCase(Locale.ROOT);
        if (!EMAIL_FORMAT.matcher(normalized).matches()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "email: must be a well-formed email address");
        }
        return normalized;
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
