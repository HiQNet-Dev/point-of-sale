package com.hiqnet.pos.sales.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.cashshift.domain.CashShiftStatus;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductModifierGroup;
import com.hiqnet.pos.catalog.domain.ProductModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.sales.domain.Sale;
import com.hiqnet.pos.sales.domain.SaleItem;
import com.hiqnet.pos.sales.domain.SaleItemModifier;
import com.hiqnet.pos.sales.domain.SaleItemModifierRepository;
import com.hiqnet.pos.sales.domain.SaleItemRepository;
import com.hiqnet.pos.sales.domain.SaleItemStatus;
import com.hiqnet.pos.sales.domain.SaleRepository;
import com.hiqnet.pos.sales.domain.SaleStatus;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.time.Clock;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.function.Function;
import java.util.stream.Collectors;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Sales application boundary. Catalog quotes are always resolved server-side;
 * the client provides product and modifier-option identities only. Sale is the
 * optimistic aggregate boundary. The sole pessimistic lock in this module is
 * on CashShift while creating a sale, coordinated with CashShift.close.
 */
@Service
public class SaleService {

    private static final BigDecimal MAX_AMOUNT = new BigDecimal("999999999999999.9999");

    private final SaleRepository saleRepository;
    private final SaleItemRepository saleItemRepository;
    private final SaleItemModifierRepository saleItemModifierRepository;
    private final ProductRepository productRepository;
    private final ProductModifierGroupRepository productModifierGroupRepository;
    private final ModifierGroupRepository modifierGroupRepository;
    private final ModifierOptionRepository modifierOptionRepository;
    private final OrganizationRepository organizationRepository;
    private final BranchRepository branchRepository;
    private final CashRegisterRepository cashRegisterRepository;
    private final CashShiftRepository cashShiftRepository;
    private final AuditPort auditPort;
    private final Clock clock;

    public SaleService(
            SaleRepository saleRepository,
            SaleItemRepository saleItemRepository,
            SaleItemModifierRepository saleItemModifierRepository,
            ProductRepository productRepository,
            ProductModifierGroupRepository productModifierGroupRepository,
            ModifierGroupRepository modifierGroupRepository,
            ModifierOptionRepository modifierOptionRepository,
            OrganizationRepository organizationRepository,
            BranchRepository branchRepository,
            CashRegisterRepository cashRegisterRepository,
            CashShiftRepository cashShiftRepository,
            AuditPort auditPort,
            Clock clock) {
        this.saleRepository = saleRepository;
        this.saleItemRepository = saleItemRepository;
        this.saleItemModifierRepository = saleItemModifierRepository;
        this.productRepository = productRepository;
        this.productModifierGroupRepository = productModifierGroupRepository;
        this.modifierGroupRepository = modifierGroupRepository;
        this.modifierOptionRepository = modifierOptionRepository;
        this.organizationRepository = organizationRepository;
        this.branchRepository = branchRepository;
        this.cashRegisterRepository = cashRegisterRepository;
        this.cashShiftRepository = cashShiftRepository;
        this.auditPort = auditPort;
        this.clock = clock;
    }

    /** Creates an empty OPEN sale after serializing with CashShift.close. */
    @Transactional
    public Sale create(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID actorUserId) {
        requireActiveOperationalParents(organizationId, branchId, cashRegisterId);
        CashShift shift = requireShiftForUpdate(organizationId, branchId, cashRegisterId, cashShiftId);
        if (shift.getStatus() != CashShiftStatus.OPEN) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CASH_SHIFT_NOT_OPEN, "The cash shift is not open");
        }
        Sale saved = saleRepository.saveAndFlush(Sale.open(organizationId, branchId, cashRegisterId, cashShiftId, actorUserId));
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("branchId", branchId.toString());
        metadata.put("cashRegisterId", cashRegisterId.toString());
        metadata.put("cashShiftId", cashShiftId.toString());
        auditPort.record(AuditRecord.ofBranch(organizationId, branchId, "sale.created", "sale", saved.getId(), AuditMetadata.of(metadata)));
        return saved;
    }

    @Transactional(readOnly = true)
    public SaleDetail get(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId) {
        Sale sale = requireSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId);
        return detail(sale);
    }

    @Transactional(readOnly = true)
    public Page<Sale> list(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, SaleStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        requireRegister(organizationId, branchId, cashRegisterId);
        requireShift(organizationId, branchId, cashRegisterId, cashShiftId);
        return status == null
                ? saleRepository.findByOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftId(organizationId, branchId, cashRegisterId, cashShiftId, pageable)
                : saleRepository.findByOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftIdAndStatus(organizationId, branchId, cashRegisterId, cashShiftId, status, pageable);
    }

    @Transactional
    public SaleDetail addItem(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId,
            long expectedSaleVersion, UUID productId, int quantity, List<UUID> modifierOptionIds, UUID actorUserId) {
        Sale sale = requireMutableSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId, expectedSaleVersion);
        Quote quote = quote(organizationId, productId, quantity, modifierOptionIds);
        SaleItem item = SaleItem.create(organizationId, saleId, quote.product().getId(), quote.product().getSku(), quote.product().getName(),
                quote.product().getUnitPrice(), quote.modifierDelta(), quote.effectiveUnitPrice(), quantity,
                saleItemRepository.findMaxPositionBySaleId(saleId) + 1);
        saleItemRepository.save(item);
        saveModifierSnapshots(item, quote.selections());
        recalculateTotal(sale);
        Sale saved = saleRepository.saveAndFlush(sale);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("productId", productId.toString());
        metadata.put("quantity", quantity);
        metadata.put("lineTotal", item.getLineTotal().toPlainString());
        auditPort.record(AuditRecord.ofBranch(organizationId, branchId, "sale.item.added", "sale", saleId, AuditMetadata.of(metadata)));
        return detail(saved);
    }

    @Transactional
    public SaleDetail updateItem(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId, UUID saleItemId,
            long expectedSaleVersion, int quantity, List<UUID> modifierOptionIds, UUID actorUserId) {
        Sale sale = requireMutableSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId, expectedSaleVersion);
        SaleItem item = saleItemRepository.findByIdAndSaleId(saleItemId, saleId)
                .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.SALE_ITEM_NOT_FOUND, "Sale item not found"));
        item.requireActive();
        Quote quote = quote(organizationId, item.getProductId(), quantity, modifierOptionIds);
        List<UUID> previousOptions = saleItemModifierRepository.findBySaleItemIdOrderByDisplayOrderAsc(saleItemId).stream()
                .map(SaleItemModifier::getModifierOptionId).sorted().toList();
        List<UUID> requestedOptions = quote.selections().stream().map(Selection::option).map(ModifierOption::getId).sorted().toList();
        if (item.getQuantity() == quantity && previousOptions.equals(requestedOptions)) {
            return detail(sale);
        }
        item.updateQuote(quote.product().getSku(), quote.product().getName(), quote.product().getUnitPrice(), quote.modifierDelta(), quote.effectiveUnitPrice(), quantity);
        // Modifiers of an ACTIVE line represent its current quote. Removed/cancelled
        // line snapshots are never touched because the state guards above reject them.
        saleItemModifierRepository.deleteBySaleItemId(saleItemId);
        saleItemModifierRepository.flush();
        saveModifierSnapshots(item, quote.selections());
        recalculateTotal(sale);
        Sale saved = saleRepository.saveAndFlush(sale);
        auditPort.record(AuditRecord.ofBranch(organizationId, branchId, "sale.item.updated", "sale", saleId,
                AuditMetadata.of(Map.of("saleItemId", saleItemId.toString(), "changedFields", List.of("quantity", "modifierOptionIds")))));
        return detail(saved);
    }

    @Transactional
    public SaleDetail removeItem(
            UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId, UUID saleItemId,
            long expectedSaleVersion, UUID actorUserId) {
        Sale sale = requireMutableSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId, expectedSaleVersion);
        SaleItem item = saleItemRepository.findByIdAndSaleId(saleItemId, saleId)
                .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.SALE_ITEM_NOT_FOUND, "Sale item not found"));
        item.remove(actorUserId, clock.instant());
        recalculateTotal(sale);
        Sale saved = saleRepository.saveAndFlush(sale);
        auditPort.record(AuditRecord.ofBranch(organizationId, branchId, "sale.item.removed", "sale", saleId,
                AuditMetadata.of("saleItemId", saleItemId.toString())));
        return detail(saved);
    }

    /** Cancellation is recovery-safe: it deliberately does not require active parents. */
    @Transactional
    public SaleDetail cancel(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId, long expectedVersion, UUID actorUserId) {
        Sale sale = requireSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId);
        if (sale.getStatus() == SaleStatus.CANCELLED) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SALE_ALREADY_CANCELLED, "The sale is already cancelled");
        }
        requireExpectedVersion(sale, expectedVersion);
        BigDecimal previousTotal = sale.getTotalAmount();
        sale.cancel(actorUserId, clock.instant());
        Sale saved = saleRepository.saveAndFlush(sale);
        auditPort.record(AuditRecord.ofBranch(organizationId, branchId, "sale.cancelled", "sale", saleId,
                AuditMetadata.of("previousTotal", previousTotal.toPlainString())));
        return detail(saved);
    }

    private Sale requireMutableSale(UUID organizationId, UUID branchId, UUID cashRegisterId, UUID cashShiftId, UUID saleId, long expectedVersion) {
        Sale sale = requireSale(organizationId, branchId, cashRegisterId, cashShiftId, saleId);
        sale.requireOpen();
        requireExpectedVersion(sale, expectedVersion);
        requireActiveOperationalParents(organizationId, branchId, cashRegisterId);
        CashShift shift = requireShift(organizationId, branchId, cashRegisterId, cashShiftId);
        if (shift.getStatus() != CashShiftStatus.OPEN) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CASH_SHIFT_NOT_OPEN, "The cash shift is not open");
        }
        return sale;
    }

    private Quote quote(UUID organizationId, UUID productId, int quantity, List<UUID> modifierOptionIds) {
        if (quantity < 1 || quantity > 1_000_000) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "quantity must be between 1 and 1000000");
        }
        Product product = productRepository.findByIdAndOrganizationId(productId, organizationId)
                .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.PRODUCT_NOT_FOUND, "Product not found"));
        if (!product.isActive()) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.PRODUCT_INACTIVE, "The product is inactive");
        }
        List<UUID> requested = modifierOptionIds == null ? List.of() : List.copyOf(modifierOptionIds);
        if (new HashSet<>(requested).size() != requested.size()) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_MODIFIER_SELECTION, "modifierOptionIds must not contain duplicates");
        }
        List<ProductModifierGroup> assignments = productModifierGroupRepository
                .findByProductIdAndOrganizationIdOrderByDisplayOrderAsc(productId, organizationId);
        Map<UUID, ModifierGroup> groups = resolveGroups(organizationId, assignments);
        Map<UUID, ModifierOption> options = resolveOptions(organizationId, requested);
        Map<UUID, List<ModifierOption>> optionsByGroup = new HashMap<>();
        for (UUID optionId : requested) {
            ModifierOption option = options.get(optionId);
            ModifierGroup group = option.getModifierGroup();
            if (!groups.containsKey(group.getId())) {
                throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_MODIFIER_SELECTION, "Modifier option is not assigned to the product");
            }
            if (!option.isActive()) {
                throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.INVALID_MODIFIER_SELECTION, "Modifier option is inactive");
            }
            optionsByGroup.computeIfAbsent(group.getId(), ignored -> new ArrayList<>()).add(option);
        }
        for (ProductModifierGroup assignment : assignments) {
            ModifierGroup group = groups.get(assignment.getModifierGroupId());
            int count = optionsByGroup.getOrDefault(group.getId(), List.of()).size();
            if (!group.isActive()) {
                if (group.getMinSelections() > 0) {
                    throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE,
                            "A required modifier group is inactive");
                }
                if (count > 0) {
                    throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.INVALID_MODIFIER_SELECTION, "Modifier group is inactive");
                }
            } else if (count < group.getMinSelections() || count > group.getMaxSelections()) {
                throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_MODIFIER_SELECTION, "Modifier selection count is outside the group bounds");
            }
        }
        List<Selection> selections = new ArrayList<>();
        for (ProductModifierGroup assignment : assignments) {
            ModifierGroup group = groups.get(assignment.getModifierGroupId());
            optionsByGroup.getOrDefault(group.getId(), List.of()).stream()
                    .sorted(Comparator.comparingInt(ModifierOption::getDisplayOrder).thenComparing(ModifierOption::getId))
                    .forEach(option -> selections.add(new Selection(group, option)));
        }
        BigDecimal delta = selections.stream().map(selection -> selection.option().getPriceDelta()).reduce(BigDecimal.ZERO, BigDecimal::add);
        BigDecimal effective = product.getUnitPrice().add(delta);
        if (effective.signum() < 0) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.NEGATIVE_EFFECTIVE_PRICE, "Modifier configuration produces a negative effective price");
        }
        validateMoney("effectiveUnitPrice", effective);
        BigDecimal lineTotal = effective.multiply(BigDecimal.valueOf(quantity));
        validateMoney("lineTotal", lineTotal);
        return new Quote(product, delta.setScale(4), effective.setScale(4), selections);
    }

    private Map<UUID, ModifierGroup> resolveGroups(UUID organizationId, List<ProductModifierGroup> assignments) {
        if (assignments.isEmpty()) return Map.of();
        List<UUID> ids = assignments.stream().map(ProductModifierGroup::getModifierGroupId).toList();
        List<ModifierGroup> found = modifierGroupRepository.findByOrganizationIdAndIdIn(organizationId, ids);
        if (found.size() != ids.size()) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE, "Product modifier configuration is unavailable");
        }
        return found.stream().collect(Collectors.toMap(ModifierGroup::getId, Function.identity()));
    }

    private Map<UUID, ModifierOption> resolveOptions(UUID organizationId, Collection<UUID> optionIds) {
        if (optionIds.isEmpty()) return Map.of();
        List<ModifierOption> found = modifierOptionRepository.findByOrganizationIdAndIdIn(organizationId, optionIds);
        if (found.size() != optionIds.size()) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_MODIFIER_SELECTION, "One or more modifier options are invalid");
        }
        return found.stream().collect(Collectors.toMap(ModifierOption::getId, Function.identity()));
    }

    private void saveModifierSnapshots(SaleItem item, List<Selection> selections) {
        List<SaleItemModifier> snapshots = new ArrayList<>();
        for (int index = 0; index < selections.size(); index++) {
            Selection selection = selections.get(index);
            snapshots.add(SaleItemModifier.snapshot(item.getOrganizationId(), item.getId(), selection.group(), selection.option(), index));
        }
        saleItemModifierRepository.saveAll(snapshots);
    }

    private void recalculateTotal(Sale sale) {
        BigDecimal total = saleItemRepository.findBySaleIdAndStatusOrderByPositionAsc(sale.getId(), SaleItemStatus.ACTIVE).stream()
                .map(SaleItem::getLineTotal).reduce(BigDecimal.ZERO, BigDecimal::add);
        validateMoney("totalAmount", total);
        sale.recalculateTotal(total, clock.instant());
    }

    private SaleDetail detail(Sale sale) {
        List<SaleItem> items = saleItemRepository.findBySaleIdOrderByPositionAsc(sale.getId());
        Map<UUID, List<SaleItemModifier>> modifiers = items.stream().collect(Collectors.toMap(SaleItem::getId,
                item -> saleItemModifierRepository.findBySaleItemIdOrderByDisplayOrderAsc(item.getId())));
        return new SaleDetail(sale, items, modifiers);
    }

    private void requireExpectedVersion(Sale sale, long expectedVersion) {
        if (sale.getVersion() != expectedVersion) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CONCURRENT_MODIFICATION, "The sale was modified concurrently; retry with fresh data");
        }
    }

    private void requireActiveOperationalParents(UUID organizationId, UUID branchId, UUID cashRegisterId) {
        Organization organization = requireOrganization(organizationId);
        if (!organization.isActive()) throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.ORGANIZATION_INACTIVE, "The organization is inactive");
        Branch branch = requireBranch(organizationId, branchId);
        if (branch.getStatus() != BranchStatus.ACTIVE) throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "The branch is inactive");
        CashRegister register = requireRegister(organizationId, branchId, cashRegisterId);
        if (!register.isActive()) throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.CASH_REGISTER_INACTIVE, "The cash register is inactive");
    }

    private Organization requireOrganization(UUID organizationId) { return organizationRepository.findById(organizationId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found")); }
    private Branch requireBranch(UUID organizationId, UUID branchId) { return branchRepository.findByIdAndOrganizationId(branchId, organizationId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found")); }
    private CashRegister requireRegister(UUID organizationId, UUID branchId, UUID registerId) { return cashRegisterRepository.findByIdAndOrganizationIdAndBranchId(registerId, organizationId, branchId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.CASH_REGISTER_NOT_FOUND, "Cash register not found")); }
    private CashShift requireShift(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId) { return cashShiftRepository.findByIdAndOrganizationIdAndBranchIdAndCashRegisterId(shiftId, organizationId, branchId, registerId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.CASH_SHIFT_NOT_FOUND, "Cash shift not found")); }
    private CashShift requireShiftForUpdate(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId) { return cashShiftRepository.findByIdAndOrganizationIdAndBranchIdAndCashRegisterIdForUpdate(shiftId, organizationId, branchId, registerId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.CASH_SHIFT_NOT_FOUND, "Cash shift not found")); }
    private Sale requireSale(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId, UUID saleId) { return saleRepository.findByIdAndOrganizationIdAndBranchIdAndCashRegisterIdAndCashShiftId(saleId, organizationId, branchId, registerId, shiftId)
            .orElseThrow(() -> new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.SALE_NOT_FOUND, "Sale not found")); }

    private static void validateMoney(String field, BigDecimal value) {
        if (value == null || value.scale() > 4 || value.precision() - value.scale() > 15 || value.compareTo(MAX_AMOUNT) > 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, field + " is out of range");
        }
    }

    public record SaleDetail(Sale sale, List<SaleItem> items, Map<UUID, List<SaleItemModifier>> modifiersByItem) {}
    private record Selection(ModifierGroup group, ModifierOption option) {}
    private record Quote(Product product, BigDecimal modifierDelta, BigDecimal effectiveUnitPrice, List<Selection> selections) {}
}
