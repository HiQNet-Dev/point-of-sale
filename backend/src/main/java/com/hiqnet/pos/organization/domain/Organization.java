package com.hiqnet.pos.organization.domain;

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
import java.time.Instant;
import java.util.Currency;
import java.util.Locale;
import java.util.UUID;
import java.util.regex.Pattern;
import org.springframework.http.HttpStatus;

/**
 * Root of configuration: owns branches and, later, users and catalog.
 * Identifier is an application-assigned UUID (never sequential/magic ids).
 */
@Entity
@Table(name = "organizations")
public class Organization {

    private static final Pattern SLUG = Pattern.compile("^[a-z0-9]+(-[a-z0-9]+)*$");
    private static final int MAX_NAME = 120;
    private static final int MAX_CODE = 64;
    private static final int MAX_TIMEZONE = 64;
    private static final int MAX_LOCALE = 20;

    @Id
    private UUID id;

    @Column(nullable = false, length = MAX_NAME)
    private String name;

    @Column(nullable = false, length = MAX_CODE, unique = true)
    private String code;

    @Column(nullable = false, length = MAX_TIMEZONE)
    private String timezone;

    @Column(nullable = false, length = 3)
    private String currency;

    @Column(nullable = false, length = MAX_LOCALE)
    private String locale;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private OrganizationStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected Organization() {
    }

    private Organization(
            UUID id, String name, String code, String timezone, String currency, String locale) {
        this.id = id;
        this.name = name;
        this.code = code;
        this.timezone = timezone;
        this.currency = currency;
        this.locale = locale;
        this.status = OrganizationStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Domain factory. Applies core invariants (code format, real IANA timezone,
     * ISO 4217 currency, BCP 47 locale) so no invalid aggregate can exist.
     * The caller is responsible for trim/lowercase normalization of {@code code}.
     */
    public static Organization create(String name, String code, String timezone, String currency, String locale) {
        validateName(name);
        validateCode(code);
        validateTimezone(timezone);
        validateCurrency(currency);
        validateLocale(locale);
        return new Organization(UUID.randomUUID(), name, code, timezone, currency.toUpperCase(Locale.ROOT), locale);
    }

    public UUID getId() {
        return id;
    }

    public String getName() {
        return name;
    }

    public String getCode() {
        return code;
    }

    public String getTimezone() {
        return timezone;
    }

    public String getCurrency() {
        return currency;
    }

    public String getLocale() {
        return locale;
    }

    public OrganizationStatus getStatus() {
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

    public void updateName(String newName) {
        validateName(newName);
        this.name = newName;
    }

    public void updateTimezone(String newTimezone) {
        validateTimezone(newTimezone);
        this.timezone = newTimezone;
    }

    public void updateCurrency(String newCurrency) {
        validateCurrency(newCurrency);
        this.currency = newCurrency.toUpperCase(Locale.ROOT);
    }

    public void updateLocale(String newLocale) {
        validateLocale(newLocale);
        this.locale = newLocale;
    }

    public void updateStatus(OrganizationStatus newStatus) {
        this.status = newStatus;
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

    private static void validateTimezone(String value) {
        if (value == null || value.isBlank()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_TIMEZONE, "timezone is required");
        }
        if (value.length() > MAX_TIMEZONE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_TIMEZONE, "timezone is too long");
        }
        try {
            java.time.ZoneId.of(value);
        } catch (java.time.DateTimeException e) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_TIMEZONE,
                    "timezone must be a valid IANA timezone, e.g. America/Mexico_City");
        }
    }

    private static void validateCurrency(String value) {
        if (value == null || value.isBlank() || value.length() != 3) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CURRENCY,
                    "currency must be a 3-letter ISO 4217 code, e.g. MXN or USD");
        }
        try {
            Currency.getInstance(value.toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException e) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_CURRENCY,
                    "currency must be a 3-letter ISO 4217 code, e.g. MXN or USD");
        }
    }

    private static void validateLocale(String value) {
        if (value == null || value.isBlank() || value.length() > MAX_LOCALE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_LOCALE,
                    "locale must be a BCP 47 tag, e.g. es-MX or en-US");
        }
        Locale parsed = Locale.forLanguageTag(value);
        if (parsed.getLanguage().isEmpty() || !parsed.toLanguageTag().equalsIgnoreCase(value)) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.INVALID_LOCALE,
                    "locale must be a BCP 47 tag, e.g. es-MX or en-US");
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