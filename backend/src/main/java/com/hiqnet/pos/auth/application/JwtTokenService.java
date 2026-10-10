package com.hiqnet.pos.auth.application;

import com.hiqnet.pos.shared.config.AuthProperties;
import java.time.Clock;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import org.springframework.security.oauth2.jose.jws.MacAlgorithm;
import org.springframework.security.oauth2.jwt.JwsHeader;
import org.springframework.security.oauth2.jwt.JwtClaimsSet;
import org.springframework.security.oauth2.jwt.JwtEncoder;
import org.springframework.security.oauth2.jwt.JwtEncoderParameters;
import org.springframework.stereotype.Service;

/**
 * Issues access tokens. A JWT is used ONLY as the short-lived access token:
 * it is never a refresh token. Claims are minimal: sub (user UUID), org_id,
 * token_type=access, jti, iss, aud, iat, exp. No password material, no
 * refresh material, no infrastructure data, and — by design — no roles,
 * permissions or branches: Roles/Permissions exist, but PostgreSQL is the
 * authorization authority and the JWT stays identity-only (ADR-0005).
 */
@Service
public class JwtTokenService {

    public static final String TOKEN_TYPE_ACCESS = "access";
    public static final String CLAIM_ORGANIZATION_ID = "org_id";
    public static final String CLAIM_TOKEN_TYPE = "token_type";

    private final JwtEncoder jwtEncoder;
    private final AuthProperties properties;
    private final Clock clock;

    public JwtTokenService(JwtEncoder jwtEncoder, AuthProperties properties, Clock clock) {
        this.jwtEncoder = jwtEncoder;
        this.properties = properties;
        this.clock = clock;
    }

    public String issueAccessToken(UUID userId, UUID organizationId) {
        Instant now = clock.instant();
        JwtClaimsSet claims = JwtClaimsSet.builder()
                .issuer(properties.issuer())
                .audience(List.of(properties.audience()))
                .subject(userId.toString())
                .claim(CLAIM_ORGANIZATION_ID, organizationId.toString())
                .claim(CLAIM_TOKEN_TYPE, TOKEN_TYPE_ACCESS)
                .issuedAt(now)
                .expiresAt(now.plus(properties.accessTokenTtl()))
                .id(UUID.randomUUID().toString())
                .build();
        return jwtEncoder
                .encode(JwtEncoderParameters.from(JwsHeader.with(MacAlgorithm.HS256).build(), claims))
                .getTokenValue();
    }

    public long accessTokenTtlSeconds() {
        return properties.accessTokenTtl().toSeconds();
    }
}