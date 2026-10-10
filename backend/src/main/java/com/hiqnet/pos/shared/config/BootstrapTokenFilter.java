package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.ApiError;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import java.io.IOException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpMethod;
import org.springframework.http.MediaType;
import org.springframework.web.filter.OncePerRequestFilter;
import tools.jackson.databind.ObjectMapper;

/**
 * Guards {@code POST /api/v1/bootstrap} at the FILTER layer: the bootstrap
 * token is verified BEFORE the request body is parsed or the controller runs,
 * so a missing/invalid token never reaches {@code BootstrapController} or
 * {@code BootstrapService}. The filter is part of the security filter chain and
 * runs for every request; it only acts on the bootstrap path and lets everything
 * else through untouched.
 *
 * <p>It deliberately creates NO {@code Authentication}: the security context
 * stays empty, so audit records the provisioning as {@code SYSTEM} (no fake
 * technical user, no fake principal). When no secret is configured the filter is
 * a no-op; the route is then closed by {@code anyRequest().denyAll()}.</p>
 */
public class BootstrapTokenFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(BootstrapTokenFilter.class);

    private static final String BOOTSTRAP_PATH = "/api/v1/bootstrap";

    private final BootstrapTokenGuard guard;
    private final ObjectMapper objectMapper;

    public BootstrapTokenFilter(BootstrapTokenGuard guard, ObjectMapper objectMapper) {
        this.guard = guard;
        this.objectMapper = objectMapper;
    }

    @Override
    protected void doFilterInternal(
            HttpServletRequest request, HttpServletResponse response, FilterChain filterChain)
            throws ServletException, IOException {
        if (!guard.enabled() || !isBootstrapRequest(request)) {
            filterChain.doFilter(request, response);
            return;
        }
        if (!guard.isValid(request.getHeader(BootstrapTokenGuard.HEADER))) {
            log.warn("security-event=bootstrap-failed reason=invalid-token path={}", request.getRequestURI());
            writeUnauthorized(response, request);
            return;
        }
        filterChain.doFilter(request, response);
    }

    private static boolean isBootstrapRequest(HttpServletRequest request) {
        return HttpMethod.POST.matches(request.getMethod()) && BOOTSTRAP_PATH.equals(request.getRequestURI());
    }

    private void writeUnauthorized(HttpServletResponse response, HttpServletRequest request) throws IOException {
        response.setStatus(HttpServletResponse.SC_UNAUTHORIZED);
        response.setContentType(MediaType.APPLICATION_JSON_VALUE);
        objectMapper.writeValue(
                response.getOutputStream(),
                ApiError.of(
                        HttpServletResponse.SC_UNAUTHORIZED,
                        ErrorCodes.BOOTSTRAP_INVALID_TOKEN,
                        "Bootstrap token is invalid or bootstrap is disabled",
                        request.getRequestURI()));
    }
}
