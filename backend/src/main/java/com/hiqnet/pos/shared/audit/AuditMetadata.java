package com.hiqnet.pos.shared.audit;

import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * Small, structured, SAFE metadata attached to an {@link AuditRecord}. It is a
 * value object with three defensive invariants that never rely on human
 * discipline:
 *
 * <ul>
 *   <li><b>Sensitive keys are rejected RECURSIVELY</b> — at EVERY level of the
 *       metadata tree, case-insensitively and normalized (ignoring {@code _},
 *       {@code -}, spaces and punctuation): {@code password}, token/hash
 *       material, {@code authorization}, JWT, secrets, credentials — including
 *       composites ({@code passwordHash}, {@code refresh_token},
 *       {@code authorizationHeader}) and values nested inside Maps or
 *       collections. Metadata is contextual business change data, never a
 *       request dump.</li>
 *   <li><b>The supported tree is restricted</b>: {@code null}, {@link String},
 *       {@link Boolean}, {@link Number}, {@code Map<String, ?>} with
 *       non-null String keys at every level, and {@link Collection} (arrays
 *       are rejected). Arbitrary POJOs, enums, UUIDs, Instants and any other
 *       type are rejected because Jackson could later serialize their
 *       properties into sensitive keys without passing this validator.</li>
 *   <li><b>Deep immutability</b>: constructing {@code AuditMetadata}
 *       deep-copies every Map/Collection into unmodifiable structures.
 *       Mutating the original inputs afterwards, or mutating anything reachable
 *       through {@link #asMap()}, changes nothing.</li>
 * </ul>
 *
 * <p>Size is bounded at the persistence boundary (see
 * {@link #MAX_SERIALIZED_BYTES}) and the DB has a matching CHECK constraint.
 * Because {@link #asMap()} returns exactly the sanitized tree that gets
 * persisted, the size check applies to the FINAL representation.</p>
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
        return new AuditMetadata(deepCopyMap(values));
    }

    public static AuditMetadata of(String key, Object value) {
        Map<String, Object> single = new LinkedHashMap<>();
        single.put(key, value);
        return of(single);
    }

    /** Empty metadata (explicitly allowed; the column stays NULL). */
    public static AuditMetadata empty() {
        return new AuditMetadata(Collections.emptyMap());
    }

    /**
     * The SANITIZED tree, deeply immutable. This is exactly the representation
     * that will be serialized and persisted (no transform can happen between
     * validation and storage), so the 16 KiB size check applies to it.
     */
    public Map<String, Object> asMap() {
        return values;
    }

    // -- recursive sanitization: validate EVERY level, build an immutable tree --

    private static Map<String, Object> deepCopyMap(Map<?, ?> source) {
        Map<String, Object> copy = new LinkedHashMap<>();
        for (Map.Entry<?, ?> entry : source.entrySet()) {
            Object rawKey = entry.getKey();
            if (rawKey == null) {
                throw new AuditException("metadata keys must not be null");
            }
            if (!(rawKey instanceof String key)) {
                throw new AuditException("metadata map keys must be strings, not " + rawKey.getClass().getName());
            }
            validateKey(key);
            copy.put(key, sanitizeValue(entry.getValue()));
        }
        return Collections.unmodifiableMap(copy);
    }

    private static List<Object> deepCopyCollection(Collection<?> source) {
        List<Object> copy = new ArrayList<>(source.size());
        for (Object item : source) {
            copy.add(sanitizeValue(item));
        }
        return Collections.unmodifiableList(copy);
    }

    private static Object sanitizeValue(Object value) {
        if (value == null) {
            return null;
        }
        if (value instanceof Map<?, ?> map) {
            return deepCopyMap(map);
        }
        if (value instanceof Collection<?> collection) {
            return deepCopyCollection(collection);
        }
        if (value.getClass().isArray()) {
            // Arrays are intentionally not supported: the contract stays small
            // and call sites have no use for them today.
            throw new AuditException("metadata arrays are not supported: " + value.getClass().getSimpleName());
        }
        if (value instanceof String || value instanceof Boolean || value instanceof Number) {
            return value;
        }
        // Arbitrary POJOs (and enums/UUID/Instant/...) could serialize into
        // sensitive keys without passing the validator: reject, never accept.
        throw new AuditException("metadata value type is not allowed: " + value.getClass().getName());
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