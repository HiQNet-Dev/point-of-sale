package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.shared.config.AuthProperties;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.security.oauth2.jose.jws.MacAlgorithm;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.jwt.JwtClaimsSet;
import org.springframework.security.oauth2.jwt.JwtDecoder;
import org.springframework.security.oauth2.jwt.JwtEncoder;
import org.springframework.security.oauth2.jwt.JwtEncoderParameters;
import org.springframework.test.context.ActiveProfiles;

/**
 * JWT rejection matrix: the decoder is verified against a real Spring
 * Security resource-server chain (signature, expiry, issuer, audience,
 * token_type). Validation never stops at the signature check.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class})
@SpringBootTest
class JwtTokenValidationTest {

    private static final org.springframework.security.oauth2.jwt.JwsHeader HEADER =
            org.springframework.security.oauth2.jwt.JwsHeader.with(MacAlgorithm.HS256).build();

    @Autowired
    private JwtEncoder jwtEncoder;

    @Autowired
    private JwtDecoder jwtDecoder;

    @Autowired
    private AuthProperties properties;

    @Test
    void validAccessTokenIsAccepted() {
        Jwt jwt = jwtDecoder.decode(encode(validClaims()));
        assertThat(jwt.getSubject()).isNotEmpty();
    }

    @Test
    void tamperedSignatureIsRejected() {
        String token = encode(validClaims());
        String[] parts = token.split("\\.");
        char[] signature = parts[2].toCharArray();
        signature[0] = signature[0] == 'A' ? 'B' : 'A';
        String tampered = parts[0] + "." + parts[1] + "." + new String(signature);

        assertThatThrownBy(() -> jwtDecoder.decode(tampered))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void expiredTokenIsRejected() {
        Instant now = Instant.now();
        JwtClaimsSet claims = clonedClaims()
                .issuedAt(now.minusSeconds(300))
                .expiresAt(now.minusSeconds(60))
                .build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void wrongIssuerIsRejected() {
        JwtClaimsSet claims = clonedClaims().issuer("attacker-issuer").build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void wrongAudienceIsRejected() {
        JwtClaimsSet claims = clonedClaims().audience(List.of("other-audience")).build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void refreshTokenTypeIsRejectedAsAccessToken() {
        JwtClaimsSet claims = clonedClaims().claim("token_type", "refresh").build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void nonUuidSubjectIsRejected() {
        JwtClaimsSet claims = clonedClaims().subject("not-a-uuid").build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void nonUuidOrganizationClaimIsRejected() {
        JwtClaimsSet claims = clonedClaims().claim("org_id", "not-a-uuid").build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void missingOrganizationClaimIsRejected() {
        JwtClaimsSet base = validClaims();
        JwtClaimsSet claims = JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(base.getAudience())
                .subject(base.getSubject())
                .claim("token_type", "access")
                .issuedAt(base.getIssuedAt())
                .expiresAt(base.getExpiresAt())
                .id(base.getId())
                .build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    @Test
    void missingSubjectIsRejected() {
        JwtClaimsSet base = validClaims();
        JwtClaimsSet claims = JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(base.getAudience())
                .claim("org_id", base.getClaimAsString("org_id"))
                .claim("token_type", "access")
                .issuedAt(base.getIssuedAt())
                .expiresAt(base.getExpiresAt())
                .id(base.getId())
                .build();
        assertThatThrownBy(() -> jwtDecoder.decode(encode(claims)))
                .isInstanceOf(org.springframework.security.oauth2.jwt.JwtException.class);
    }

    private JwtClaimsSet validClaims() {
        Instant now = Instant.now();
        return JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(List.of(properties.audience()))
                .subject(UUID.randomUUID().toString())
                .claim("org_id", UUID.randomUUID().toString())
                .claim("token_type", "access")
                .issuedAt(now)
                .expiresAt(now.plusSeconds(600))
                .id(UUID.randomUUID().toString())
                .build();
    }

    private JwtClaimsSet.Builder clonedClaims() {
        JwtClaimsSet base = validClaims();
        return JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(base.getAudience())
                .subject(base.getSubject())
                .claim("org_id", base.getClaimAsString("org_id"))
                .claim("token_type", base.getClaimAsString("token_type"))
                .issuedAt(base.getIssuedAt())
                .expiresAt(base.getExpiresAt())
                .id(base.getId());
    }

    private String encode(JwtClaimsSet claims) {
        return jwtEncoder.encode(JwtEncoderParameters.from(HEADER, claims)).getTokenValue();
    }
}