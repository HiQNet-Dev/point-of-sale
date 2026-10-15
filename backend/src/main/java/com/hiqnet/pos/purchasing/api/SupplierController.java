package com.hiqnet.pos.purchasing.api;

import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.application.UpdateSupplierCommand;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.Parameter;
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
 * API administrativa de proveedores propiedad de una organización.
 *
 * <p>Las rutas se autentican de forma explícita en {@code SecurityConfig} y la
 * autorización fina se decide por permisos organization-scoped en PostgreSQL.
 * No hay DELETE: {@code INACTIVE} conserva las futuras referencias históricas.</p>
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/suppliers")
@Tag(name = "Purchasing Suppliers", description = "Administración de proveedores de compras por organización.")
public class SupplierController {

    private static final int MAX_PAGE_SIZE = 100;
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final SupplierService supplierService;

    public SupplierController(SupplierService supplierService) {
        this.supplierService = supplierService;
    }

    /**
     * Registra un proveedor activo dentro de la organización autorizada.
     *
     * @param organizationId identificador de la organización
     * @param request datos validados del proveedor
     * @return proveedor creado
     */
    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(
            summary = "Registrar un proveedor",
            description = "Requiere `purchasing.supplier.write` en la organización. El código es un slug único e inmutable.")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Proveedor creado"),
        @ApiResponse(responseCode = "400", description = "Solicitud inválida (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "401", description = "Autenticación requerida (AUTHENTICATION_REQUIRED)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Permiso purchasing.supplier.write ausente", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización inexistente (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Código duplicado (SUPPLIER_CODE_ALREADY_EXISTS)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'purchasing.supplier.write')")
    public SupplierResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateSupplierRequest request) {
        return SupplierResponse.from(supplierService.create(
                organizationId,
                request.code(),
                request.name(),
                request.legalName(),
                request.taxId(),
                request.contactName(),
                request.email(),
                request.phone(),
                request.address()));
    }

    /**
     * Lista proveedores dentro de la organización con orden fijo y paginación.
     *
     * @param organizationId identificador de la organización
     * @param page índice de página base cero
     * @param size elementos por página
     * @param status estado operativo opcional
     * @param q texto de búsqueda opcional
     * @return página de proveedores
     */
    @GetMapping
    @Operation(
            summary = "Listar proveedores",
            description = "Requiere `purchasing.supplier.read`. Filtra por estado y por texto en código, nombre, razón social, identificador fiscal o contacto; orden fijo name ASC, id ASC.")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Página de proveedores"),
        @ApiResponse(responseCode = "400", description = "Parámetros de paginación inválidos (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "Autenticación requerida (AUTHENTICATION_REQUIRED)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Permiso purchasing.supplier.read ausente", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización inexistente (ORGANIZATION_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'purchasing.supplier.read')")
    public PageResponse<SupplierResponse> list(
            @PathVariable UUID organizationId,
            @Parameter(description = "Página base cero.", example = "0") @RequestParam(defaultValue = "0") int page,
            @Parameter(description = "Tamaño entre 1 y 100.", example = "20") @RequestParam(defaultValue = "20") int size,
            @Parameter(description = "Estado operativo opcional.", example = "ACTIVE") @RequestParam(required = false) SupplierStatus status,
            @Parameter(description = "Texto de búsqueda opcional.", example = "centro") @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<Supplier> suppliers = supplierService.list(organizationId, status, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(suppliers, SupplierResponse::from);
    }

    /**
     * Obtiene un proveedor dentro del ámbito de la organización.
     *
     * @param organizationId identificador de la organización
     * @param supplierId identificador del proveedor
     * @return proveedor solicitado
     */
    @GetMapping("/{supplierId}")
    @Operation(
            summary = "Consultar un proveedor",
            description = "Requiere `purchasing.supplier.read`; el proveedor se resuelve únicamente dentro de la organización de la ruta.")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Proveedor encontrado"),
        @ApiResponse(responseCode = "401", description = "Autenticación requerida (AUTHENTICATION_REQUIRED)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Permiso purchasing.supplier.read ausente", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización o proveedor inexistente (ORGANIZATION_NOT_FOUND, SUPPLIER_NOT_FOUND)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'purchasing.supplier.read')")
    public SupplierResponse get(@PathVariable UUID organizationId, @PathVariable UUID supplierId) {
        return SupplierResponse.from(supplierService.get(organizationId, supplierId));
    }

    /**
     * Actualiza los datos editables de un proveedor con control de versión.
     *
     * @param organizationId identificador de la organización
     * @param supplierId identificador del proveedor
     * @param request datos parciales y versión esperada
     * @return proveedor resultante
     */
    @PatchMapping("/{supplierId}")
    @Operation(
            summary = "Actualizar un proveedor",
            description = "Requiere `purchasing.supplier.write`. `expectedVersion` evita perder ediciones concurrentes; el código no puede modificarse.")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Proveedor actualizado"),
        @ApiResponse(responseCode = "400", description = "Solicitud inválida (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "Autenticación requerida (AUTHENTICATION_REQUIRED)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Permiso purchasing.supplier.write ausente", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organización o proveedor inexistente (ORGANIZATION_NOT_FOUND, SUPPLIER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Versión obsoleta (CONCURRENT_MODIFICATION)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'purchasing.supplier.write')")
    public SupplierResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID supplierId,
            @Valid @RequestBody UpdateSupplierRequest request) {
        return SupplierResponse.from(supplierService.update(
                organizationId,
                supplierId,
                new UpdateSupplierCommand(
                        request.expectedVersion(),
                        request.name(),
                        request.legalName(),
                        request.taxId(),
                        request.contactName(),
                        request.email(),
                        request.phone(),
                        request.address(),
                        request.status())));
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}
