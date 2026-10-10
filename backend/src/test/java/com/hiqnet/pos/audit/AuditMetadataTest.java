package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

/**
 * Defensive metadata safety: sensitive keys (passwords, tokens, Authorization,
 * JWT, secrets) are rejected BEFORE anything can reach the database —
 * case-insensitively, for obvious composites, and RECURSIVELY at every level
 * of the metadata tree (nested Maps, Maps inside Lists, mixed nesting). The
 * supported tree is restricted (no arbitrary POJOs/arrays) and deeply
 * immutable: mutating the original inputs or anything reachable through
 * {@code asMap()} changes nothing.
 */
class AuditMetadataTest {

    @ParameterizedTest
    @ValueSource(
            strings = {
                "password",
                "Password",
                "passwordHash",
                "PASSWORD_HASH",
                "accessToken",
                "access_token",
                "refreshToken",
                "refresh_token",
                "Authorization",
                "authorizationHeader",
                "tokenHash",
                "token",
                "jwt",
                "secret",
                "SeCrEt",
                "clientSecret",
                "privateKey",
                "credentials"
            })
    void rejectsSensitiveKeys(String key) {
        assertThatThrownBy(() -> AuditMetadata.of(key, "boom"))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsSensitiveKeysInsideNestedMaps() {
        assertThatThrownBy(() -> AuditMetadata.of("context", Map.of("password", "x")))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsDeepNestedAndCaseInsensitiveSensitiveKeys() {
        assertThatThrownBy(() -> AuditMetadata.of(
                        "security", Map.of("nested", Map.of("Authorization", "Bearer ..."))))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsSensitiveKeysInsideMapsInsideLists() {
        assertThatThrownBy(() -> AuditMetadata.of("changes", List.of(Map.of("refresh_token", "x"))))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsMixedNestingWithCompositeSensitiveKeys() {
        assertThatThrownBy(() -> AuditMetadata.of(
                        "items",
                        List.of(Map.of("detail", Map.of("clientSecret", "x")))))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void acceptsLegitimateContextualKeys() {
        Map<String, Object> values = new LinkedHashMap<>();
        values.put("changedFields", List.of("name", "timezone"));
        values.put("fromStatus", "ACTIVE");
        values.put("toStatus", "INACTIVE");
        values.put("permissionCount", 4);
        values.put("roleCount", 2);
        values.put("code", "manager");

        AuditMetadata metadata = AuditMetadata.of(values);
        assertThat(metadata.asMap()).containsEntry("permissionCount", 4);
    }

    @Test
    void acceptsAllowedNestedData() {
        Map<String, Object> values = new LinkedHashMap<>();
        values.put("changedFields", List.of("name", "timezone"));
        values.put("before", Map.of("status", "ACTIVE"));
        values.put("after", Map.of("status", "INACTIVE"));

        AuditMetadata metadata = AuditMetadata.of(values);

        assertThat(metadata.asMap()).containsEntry("before", Map.of("status", "ACTIVE"));
        assertThat(metadata.asMap()).containsEntry("after", Map.of("status", "INACTIVE"));
    }

    @Test
    void rejectsNullKeys() {
        Map<String, Object> values = new LinkedHashMap<>();
        values.put(null, "value");
        assertThatThrownBy(() -> AuditMetadata.of(values)).isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsNullKeysInsideNestedMaps() {
        Map<String, Object> nested = new LinkedHashMap<>();
        nested.put(null, "value");
        assertThatThrownBy(() -> AuditMetadata.of("context", nested)).isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsNonStringNestedKeys() {
        // A non-String key must never be silently converted by Jackson later.
        assertThatThrownBy(() -> AuditMetadata.of("context", Map.of(1, "x")))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsArbitraryJsonArrays() {
        assertThatThrownBy(() -> AuditMetadata.of("items", new String[] {"a"}))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void rejectsArbitraryPojoValues() {
        assertThatThrownBy(() -> AuditMetadata.of("context", new SomePayload("value")))
                .isInstanceOf(AuditException.class);
        assertThatThrownBy(() -> AuditMetadata.of("context", Map.of("nested", new SomePayload("value"))))
                .isInstanceOf(AuditException.class);
    }

    @Test
    void isImmutableAndProtectsInnerMutation() {
        Map<String, Object> input = new LinkedHashMap<>();
        input.put("code", "manager");
        AuditMetadata metadata = AuditMetadata.of(input);

        assertThatThrownBy(() -> metadata.asMap().put("x", "y"))
                .isInstanceOf(UnsupportedOperationException.class);
        assertThat(metadata.asMap()).containsExactlyEntriesOf(Map.of("code", "manager"));
    }

    @Test
    void mutatingOriginalStructuresDoesNotAffectMetadata() {
        List<String> changedFields = new ArrayList<>(List.of("name"));
        Map<String, Object> before = new LinkedHashMap<>();
        before.put("status", "ACTIVE");
        Map<String, Object> root = new LinkedHashMap<>();
        root.put("changedFields", changedFields);
        root.put("before", before);

        AuditMetadata metadata = AuditMetadata.of(root);

        changedFields.add("password");
        before.put("status", "INACTIVE");
        root.put("extra", "x");

        assertThat(metadata.asMap())
                .containsExactlyInAnyOrderEntriesOf(Map.of(
                        "changedFields", List.of("name"),
                        "before", Map.of("status", "ACTIVE")));
    }

    @Test
    void asMapIsDeeplyImmutable() {
        Map<String, Object> root = new LinkedHashMap<>();
        root.put("changedFields", new ArrayList<>(List.of("name")));
        root.put("before", new LinkedHashMap<>(Map.of("status", "ACTIVE")));

        AuditMetadata metadata = AuditMetadata.of(root);
        Map<String, Object> view = metadata.asMap();

        assertThatThrownBy(() -> view.put("x", "y"))
                .isInstanceOf(UnsupportedOperationException.class);
        assertThatThrownBy(() -> ((java.util.List) view.get("changedFields")).add("x"))
                .isInstanceOf(UnsupportedOperationException.class);
        assertThatThrownBy(() -> ((java.util.Map) view.get("before")).put("k", "v"))
                .isInstanceOf(UnsupportedOperationException.class);
    }

    @Test
    void emptyMetadataIsAllowed() {
        assertThat(AuditMetadata.empty().asMap()).isEmpty();
    }

    /** A deliberately unsafe payload: must never be serialized into audit. */
    private record SomePayload(String name) {}
}