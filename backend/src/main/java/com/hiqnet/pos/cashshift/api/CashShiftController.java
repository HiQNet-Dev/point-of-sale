package com.hiqnet.pos.cashshift.api;

import com.hiqnet.pos.cashshift.application.CashShiftService;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Branch/register-scoped cash shifts. A shift belongs to a register of the
 * branch. Reads and close authorize with
 * {@code hasBranchPermissionIgnoringBranchStatus} so an already-open shift stays
 * reachable (recovery) after the branch is deactivated; opening keeps
 * {@code hasBranchPermission}, which DOES require the branch ACTIVE. Both never
 * promote to organization scope. There is no update or delete: a shift is
 * opened and later closed, and a CLOSED shift is immutable.
 */
@RestController
@RequestMapping(
        "/api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}/cash-shifts")
@Tag(name = "Cash Shifts", description = "Turnos (shifts) de una caja.")
public class CashShiftController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: the most recent shift first (openedAt DESC, id DESC). */
    private static final Sort ORDER = Sort.by(Sort.Order.desc("openedAt"), Sort.Order.desc("id"));

    private final CashShiftService cashShiftService;

    public CashShiftController(CashShiftService cashShiftService) {
        this.cashShiftService = cashShiftService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Open a cash shift on a cash register (at most one OPEN shift per register)")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Opened", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid amount (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or register not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Register/branch inactive or an OPEN shift already exists (CASH_REGISTER_INACTIVE, BRANCH_INACTIVE, CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-shift.open)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'cash-shift.open')")
    public CashShiftResponse open(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId,
            @AuthenticationPrincipal Jwt jwt,
            @Valid @RequestBody OpenCashShiftRequest request) {
        return CashShiftResponse.from(cashShiftService.open(
                organizationId, branchId, cashRegisterId, actorId(jwt), request.openingAmount()));
    }

    @GetMapping
    @Operation(summary = "List cash shifts of a cash register (paginated; filter status)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or register not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-shift.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'cash-shift.read')")
    public PageResponse<CashShiftResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) CashShiftStatus status) {
        validatePaging(page, size);
        Page<CashShift> shifts = cashShiftService.list(
                organizationId, branchId, cashRegisterId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(shifts, CashShiftResponse::from);
    }

    @GetMapping("/{cashShiftId}")
    @Operation(summary = "Get a cash shift by id (scoped to the organization, branch and register)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch, register or shift not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND, CASH_SHIFT_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-shift.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'cash-shift.read')")
    public CashShiftResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId,
            @PathVariable UUID cashShiftId) {
        return CashShiftResponse.from(cashShiftService.get(organizationId, branchId, cashRegisterId, cashShiftId));
    }

    @PostMapping("/{cashShiftId}/close")
    @Operation(summary = "Close an open cash shift (declared closing amount; optimistic concurrency)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Closed", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid amount or missing expectedVersion (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch, register or shift not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND, CASH_SHIFT_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Already closed or concurrent modification (CASH_SHIFT_ALREADY_CLOSED, CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-shift.close)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermissionIgnoringBranchStatus(authentication, #organizationId, #branchId, 'cash-shift.close')")
    public CashShiftResponse close(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId,
            @PathVariable UUID cashShiftId,
            @AuthenticationPrincipal Jwt jwt,
            @Valid @RequestBody CloseCashShiftRequest request) {
        return CashShiftResponse.from(cashShiftService.close(
                organizationId,
                branchId,
                cashRegisterId,
                cashShiftId,
                request.expectedVersion(),
                request.declaredCashAmount(),
                actorId(jwt)));
    }

    private static UUID actorId(Jwt jwt) {
        if (jwt == null || jwt.getSubject() == null) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        }
        try {
            return UUID.fromString(jwt.getSubject());
        } catch (IllegalArgumentException e) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.AUTHENTICATION_REQUIRED, "Authenticated user is required");
        }
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
