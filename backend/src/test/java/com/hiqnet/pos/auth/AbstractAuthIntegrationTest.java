package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.auth.application.AuthService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.UUID;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;
import tools.jackson.databind.ObjectMapper;

/**
 * Shared fixture base for auth integration suites. Boots the REAL security
 * configuration (no TestSecurityConfig): public/authenticated/deny semantics
 * are the production ones. Testcontainers provides a real PostgreSQL.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class})
@SpringBootTest
@AutoConfigureMockMvc
public abstract class AbstractAuthIntegrationTest {

    protected static final String PASSWORD = "Correct-horse-battery-staple-1";

    @Autowired
    protected MockMvc mockMvc;

    @Autowired
    protected ObjectMapper objectMapper;

    @Autowired
    protected OrganizationRepository organizationRepository;

    @Autowired
    protected UserRepository userRepository;

    @Autowired
    protected PasswordEncoder passwordEncoder;

    @Autowired
    protected AuthService authService;

    protected String createOrganizationWithCode(String code) {
        Organization organization =
                Organization.create("Org " + code, code, "America/Mexico_City", "MXN", "es-MX");
        organizationRepository.saveAndFlush(organization);
        return organization.getId().toString();
    }

    protected String createOrganization() {
        return createOrganizationWithCode("org-" + UUID.randomUUID().toString().substring(0, 8));
    }

    protected User createUserForOrganization(String organizationId, String email, String password) {
        User user = User.create(
                UUID.fromString(organizationId),
                email,
                passwordEncoder.encode(password));
        return userRepository.saveAndFlush(user);
    }

    protected User createActiveUser(String email, String password) {
        return createUserForOrganization(createOrganization(), email, password);
    }

    protected String loginJson(String organizationCode, String email, String password) {
        return """
                {"organizationCode":"%s","email":"%s","password":"%s"}
                """.formatted(organizationCode, email, password);
    }

    protected String postLoginExpectingToken(String organizationCode, String email, String password)
            throws Exception {
        String body = loginJson(organizationCode, email, password);
        String response = mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
        return objectMapper.readTree(response).get("accessToken").asText();
    }

    protected String loginAndGetRefreshToken(String organizationCode, String email, String password)
            throws Exception {
        String body = loginJson(organizationCode, email, password);
        String response = mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
        return objectMapper.readTree(response).get("refreshToken").asText();
    }

    protected String refreshRawToken(String refreshToken) throws Exception {
        String body = """
                {"refreshToken":"%s"}
                """.formatted(refreshToken);
        String response = mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
        return objectMapper.readTree(response).get("refreshToken").asText();
    }

    protected String emailFor(User user) {
        return user.getEmail().toLowerCase(Locale.ROOT);
    }

    protected String uniqueEmail(String prefix) {
        String safePrefix = prefix.replace("@", "-");
        return safePrefix + UUID.randomUUID().toString().substring(0, 8) + "@example.com";
    }
}