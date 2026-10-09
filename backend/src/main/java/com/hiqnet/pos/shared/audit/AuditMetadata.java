package com.hiqnet.pos.shared.audit;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * Small, structured, SAFE metadata attached to an {@link AuditRecord}. It is a
 * value object with two defensive invariants that never rely on human
 * discipline:
 *
 * <ul>
 *   <li><b>Sensitive keys are rejected case-insensitively</b> (password,
 *       token/hash material, authorization, JWT, secrets, credentials) before
 *       anything can be persisted — including obvious composites
 *       ({@code passwordHash}, {@code refresh_token}, {@code authorizationHeader}).
 *       Metadata is contextual business change data, never a request dump.</li>
 *   <li><b>Size is bounded at the persistence boundary</b> (see
 *       {@link #MAX_SERIALIZED_BYTES}) — the serializer checks the actual JSON
 *       length before the insert; the DB has a matching CHECK constraint.</li>
 * </ul>
 *
 * <p>Immutability: {@link #asMap()} returns an unmodifiable copy.</p>
 */
public final class AuditMetadata {

    /** Upper bound of the serialized JSON — audit is not arbitrary storage. */
    public static final int MAX_SERIALIZED_BYTES = 16 * 1024;

    private static final Set<String> FORBIDDEN_TOKENS = Set.of(
            "password",
            "token",
            "authorization",
            "jwt",
            "secret",
            "privatekey",
            "credential");

    private final Map<String, Object> values;

    private AuditMetadata(Map<String, Object> values) {
        this.values = values;
    }

    public static AuditMetadata of(Map<String, Object> values) {
        if (values == null) {
            throw new AuditException("metadata must not be null");
        }
        Map<String, Object> copy = new LinkedHashMap<>();
        for (Map.Entry<String, Object> entry : values.entrySet()) {
            String key = entry.getKey();
            if (key == null) {
                throw new AuditException("metadata keys must not be null");
            }
            validateKey(key);
            copy.put(key, entry.getValue());
        }
        return new AuditMetadata(Collections.unmodifiableMap(copy));
    }

    public static AuditMetadata of(String key, Object value) {
        return of(Map.of(key, value));
    }

    /** Empty metadata (explicitly allowed; the column stays NULL). */
    public static AuditMetadata empty() {
        return new AuditMetadata(Collections.emptyMap());
    }

    public Map<String, Object> asMap() {
        return values;
    }

    private static void validateKey(String key) {
        // Normalize: lowercase + strip every non-alphanumeric character, then
        // look for a forbidden token anywhere in the key (composites hit the
        // same filter as their plain form).
        String normalized = key.toLowerCase(Locale.ROOT).replaceAll("[^a-z0-9]", "");
        for (String token : FORBIDDEN_TOKENS) {
            if (normalized.contains(token)) {
                throw new AuditException(
                        "metadata key '" + key + "' is not allowed (sensitive field)");
            }
        }
    }
}