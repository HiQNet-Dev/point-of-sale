package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.shared.config.AuthProperties;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.oauth2.jose.jws.MacAlgorithm;
import org.springframework.security.oauth2.jwt.JwsHeader;
import org.springframework.security.oauth2.jwt.JwtClaimsSet;
import org.springframework.security.oauth2.jwt.JwtEncoder;
import org.springframework.security.oauth2.jwt.JwtEncoderParameters;

/**
 * End-to-end proof that a token which is correctly signed but structurally
 * malformed (non-UUID {@code sub} or {@code org_id}) is rejected by the
 * security layer as 401 TOKEN_INVALID, never reaching a controller and
 * producing a 500 from {@code UUID.fromString(...)}.
 */
class JwtClaimIntegrationTest extends AbstractAuthIntegrationTest {

    @Autowired
    private JwtEncoder jwtEncoder;

    @Autowired
    private AuthProperties properties;

    @Test
    void tokenWithNonUuidSubjectIsRejectedBySecurity() throws Exception {
        String token = forgeToken("not-a-uuid", UUID.randomUUID().toString());

        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", "Bearer " + token))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("TOKEN_INVALID"));
    }

    @Test
    void tokenWithNonUuidOrganizationClaimIsRejectedBySecurity() throws Exception {
        String token = forgeToken(UUID.randomUUID().toString(), "nope");

        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", "Bearer " + token))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("TOKEN_INVALID"));
    }

    @Test
    void expiredAccessTokenIsRejectedAsTokenExpiredWithJsonBody() throws Exception {
        Instant now = Instant.now();
        String token = forgeToken(
                UUID.randomUUID().toString(),
                UUID.randomUUID().toString(),
                now.minusSeconds(600),
                now.minusSeconds(60));

        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", "Bearer " + token))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("TOKEN_EXPIRED"));
    }

    private String forgeToken(String subject, String organizationId) {
        Instant now = Instant.now();
        return forgeToken(subject, organizationId, now, now.plusSeconds(600));
    }

    private String forgeToken(
            String subject, String organizationId, Instant issuedAt, Instant expiresAt) {
        JwtClaimsSet claims = JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(List.of(properties.audience()))
                .subject(subject)
                .claim("org_id", organizationId)
                .claim("token_type", "access")
                .issuedAt(issuedAt)
                .expiresAt(expiresAt)
                .id(UUID.randomUUID().toString())
                .build();
        return jwtEncoder
                .encode(JwtEncoderParameters.from(JwsHeader.with(MacAlgorithm.HS256).build(), claims))
                .getTokenValue();
    }
}
