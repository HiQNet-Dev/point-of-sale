package com.hiqnet.pos.shared.config;

import io.swagger.v3.oas.annotations.enums.SecuritySchemeType;
import io.swagger.v3.oas.annotations.security.SecurityScheme;
import java.time.Clock;
import java.util.Base64;
import java.util.UUID;
import javax.crypto.SecretKey;
import javax.crypto.spec.SecretKeySpec;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.security.crypto.bcrypt.BCryptPasswordEncoder;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.security.oauth2.core.DelegatingOAuth2TokenValidator;
import org.springframework.security.oauth2.core.OAuth2Error;
import org.springframework.security.oauth2.core.OAuth2TokenValidator;
import org.springframework.security.oauth2.core.OAuth2TokenValidatorResult;
import org.springframework.security.oauth2.jose.jws.MacAlgorithm;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.jwt.JwtDecoder;
import org.springframework.security.oauth2.jwt.JwtEncoder;
import org.springframework.security.oauth2.jwt.JwtValidators;
import org.springframework.security.oauth2.jwt.NimbusJwtDecoder;
import org.springframework.security.oauth2.jwt.NimbusJwtEncoder;
import com.nimbusds.jose.jwk.source.ImmutableSecret;

/**
 * Authentication beans: password hashing (BCrypt), JWT signing/validation
 * (Spring Security + Nimbus JOSE — no third-party JWT library), and the
 * OpenAPI bearer scheme. The decoder validates signature, expiry, issuer,
 * audience, the custom {@code token_type} claim and the STRUCTURAL shape of
 * {@code sub}/{@code org_id} (both must be UUIDs); validation never stops at
 * the signature check.
 */
@Configuration
@EnableConfigurationProperties(AuthProperties.class)
@SecurityScheme(
        name = "bearerAuth",
        type = SecuritySchemeType.HTTP,
        scheme = "bearer",
        bearerFormat = "JWT",
        description = "Short-lived JWT access token (10 minutes). Obtain via POST /api/v1/auth/login.")
public class AuthConfig {

    /** HS256 requires >= 256 bits of key material. */
    private static final int MIN_SECRET_BYTES = 32;

    private static final String CLAIM_ORGANIZATION_ID = "org_id";

    @Bean
    public Clock clock() {
        return Clock.systemUTC();
    }

    @Bean
    public PasswordEncoder passwordEncoder(AuthProperties properties) {
        return new BCryptPasswordEncoder(properties.passwordStrength());
    }

    /**
     * HS256 with the configured secret. {@code AUTH_JWT_SECRET} is interpreted
     * as a Base64-encoded key (e.g. {@code openssl rand -base64 32}): it is
     * decoded and must yield at least 32 real bytes, otherwise application
     * startup fails. There is no default or fallback secret.
     */
    @Bean
    public SecretKey jwtSecretKey(AuthProperties properties) {
        byte[] keyBytes;
        try {
            keyBytes = Base64.getDecoder().decode(properties.jwtSecret());
        } catch (IllegalArgumentException e) {
            throw new IllegalStateException(
                    "AUTH_JWT_SECRET must be a Base64-encoded key; generate one with `openssl rand -base64 32`",
                    e);
        }
        if (keyBytes.length < MIN_SECRET_BYTES) {
            throw new IllegalStateException(
                    "AUTH_JWT_SECRET must decode to at least " + MIN_SECRET_BYTES + " bytes (256 bits) for HS256");
        }
        return new SecretKeySpec(keyBytes, "HmacSHA256");
    }

    @Bean
    public JwtEncoder jwtEncoder(SecretKey jwtSecretKey) {
        return new NimbusJwtEncoder(new ImmutableSecret<>(jwtSecretKey));
    }

    @Bean
    public JwtDecoder jwtDecoder(SecretKey jwtSecretKey, AuthProperties properties) {
        NimbusJwtDecoder decoder =
                NimbusJwtDecoder.withSecretKey(jwtSecretKey).macAlgorithm(MacAlgorithm.HS256).build();
        decoder.setJwtValidator(new DelegatingOAuth2TokenValidator<>(
                JwtValidators.createDefaultWithIssuer(properties.issuer()),
                new AudienceJwtValidator(properties.audience()),
                new TokenTypeJwtValidator(),
                new RequiredClaimsJwtValidator()));
        return decoder;
    }

    static final class AudienceJwtValidator implements OAuth2TokenValidator<Jwt> {

        private final String audience;

        AudienceJwtValidator(String audience) {
            this.audience = audience;
        }

        @Override
        public OAuth2TokenValidatorResult validate(Jwt jwt) {
            if (jwt.getAudience() != null && jwt.getAudience().contains(audience)) {
                return OAuth2TokenValidatorResult.success();
            }
            return OAuth2TokenValidatorResult.failure(
                    new OAuth2Error("invalid_audience", "JWT audience does not match", null));
        }
    }

    static final class TokenTypeJwtValidator implements OAuth2TokenValidator<Jwt> {

        private static final String ACCESS = "access";

        @Override
        public OAuth2TokenValidatorResult validate(Jwt jwt) {
            if (ACCESS.equals(jwt.getClaimAsString("token_type"))) {
                return OAuth2TokenValidatorResult.success();
            }
            return OAuth2TokenValidatorResult.failure(
                    new OAuth2Error("invalid_token_type", "JWT token_type must be 'access'", null));
        }
    }

    /**
     * Structural validation of the claims the application relies on: a token
     * that is correctly signed but carries a non-UUID {@code sub} or
     * {@code org_id} (e.g. {@code sub="abc"}) must be rejected here, as an
     * authentication failure, instead of reaching the controller and blowing up
     * on {@code UUID.fromString(...)}.
     */
    static final class RequiredClaimsJwtValidator implements OAuth2TokenValidator<Jwt> {

        @Override
        public OAuth2TokenValidatorResult validate(Jwt jwt) {
            if (isUuid(jwt.getSubject()) && isUuid(jwt.getClaimAsString(CLAIM_ORGANIZATION_ID))) {
                return OAuth2TokenValidatorResult.success();
            }
            return OAuth2TokenValidatorResult.failure(
                    new OAuth2Error("invalid_claims", "JWT must carry a UUID sub and org_id", null));
        }

        private static boolean isUuid(String value) {
            if (value == null || value.isBlank()) {
                return false;
            }
            try {
                UUID.fromString(value);
                return true;
            } catch (IllegalArgumentException e) {
                return false;
            }
        }
    }
}