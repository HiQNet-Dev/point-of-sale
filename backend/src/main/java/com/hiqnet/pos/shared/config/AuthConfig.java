package com.hiqnet.pos.shared.config;

import io.swagger.v3.oas.annotations.enums.SecuritySchemeType;
import io.swagger.v3.oas.annotations.security.SecurityScheme;
import java.nio.charset.StandardCharsets;
import java.time.Clock;
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
 * audience and the custom {@code token_type} claim; validation never stops at
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

    @Bean
    public Clock clock() {
        return Clock.systemUTC();
    }

    @Bean
    public PasswordEncoder passwordEncoder(AuthProperties properties) {
        return new BCryptPasswordEncoder(properties.passwordStrength());
    }

    /**
     * HS256 with the configured secret. The secret is validated at startup
     * (>= 32 bytes real entropy, no default value).
     */
    @Bean
    public SecretKey jwtSecretKey(AuthProperties properties) {
        return new SecretKeySpec(properties.jwtSecret().getBytes(StandardCharsets.UTF_8), "HmacSHA256");
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
                new TokenTypeJwtValidator()));
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
}