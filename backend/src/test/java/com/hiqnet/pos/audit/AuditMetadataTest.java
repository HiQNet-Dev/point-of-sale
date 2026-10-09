package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

/**
 * Defensive metadata safety: sensitive keys (passwords, tokens, Authorization,
 * JWT, secrets) are rejected BEFORE anything can reach the database —
 * case-insensitively and for obvious composites — while legitimate contextual
 * keys pass. The values are immutable once built.
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
    void rejectsNullKeys() {
        Map<String, Object> values = new LinkedHashMap<>();
        values.put(null, "value");
        assertThatThrownBy(() -> AuditMetadata.of(values)).isInstanceOf(AuditException.class);
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
    void emptyMetadataIsAllowed() {
        assertThat(AuditMetadata.empty().asMap()).isEmpty();
    }
}