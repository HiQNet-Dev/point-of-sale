package com.hiqnet.pos.cashregister.api;

import com.hiqnet.pos.cashregister.application.CashRegisterService;
import com.hiqnet.pos.cashregister.application.UpdateCashRegisterCommand;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Branch-scoped cash-register administration. Cash registers belong to the
 * branch, so every endpoint authorizes with branch-scope
 * {@code hasBranchPermission} (org-wide grant OR an ACTIVE membership of the
 * exact branch — never promoted to organization scope). There is no DELETE: a
 * register is retired via status INACTIVE.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers")
public class CashRegisterController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: name ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final CashRegisterService cashRegisterService;

    public CashRegisterController(CashRegisterService cashRegisterService) {
        this.cashRegisterService = cashRegisterService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a cash register inside a branch")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or branch not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (CASH_REGISTER_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-register.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'cash-register.create')")
    public CashRegisterResponse create(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Valid @RequestBody CreateCashRegisterRequest request) {
        return CashRegisterResponse.from(
                cashRegisterService.create(organizationId, branchId, request.code(), request.name()));
    }

    @GetMapping
    @Operation(summary = "List cash registers of a branch (paginated; filters status, q)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or branch not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-register.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'cash-register.read')")
    public PageResponse<CashRegisterResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) CashRegisterStatus status,
            @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<CashRegister> registers =
                cashRegisterService.list(organizationId, branchId, status, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(registers, CashRegisterResponse::from);
    }

    @GetMapping("/{cashRegisterId}")
    @Operation(summary = "Get a cash register by id (scoped to the organization and branch)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or cash register not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-register.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'cash-register.read')")
    public CashRegisterResponse get(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId) {
        return CashRegisterResponse.from(cashRegisterService.get(organizationId, branchId, cashRegisterId));
    }

    @PatchMapping("/{cashRegisterId}")
    @Operation(summary = "Partially update a cash register (code immutable; name, status)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or cash register not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, CASH_REGISTER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (cash-register.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasBranchPermission(authentication, #organizationId, #branchId, 'cash-register.update')")
    public CashRegisterResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID cashRegisterId,
            @Valid @RequestBody UpdateCashRegisterRequest request) {
        return CashRegisterResponse.from(cashRegisterService.update(
                organizationId,
                branchId,
                cashRegisterId,
                new UpdateCashRegisterCommand(request.expectedVersion(), request.name(), request.status())));
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
