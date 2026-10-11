package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import com.hiqnet.pos.catalog.domain.Product;
import java.math.BigDecimal;
import java.util.List;
import java.util.UUID;

/** Current product and assigned modifier graph used to construct a SaleItem request. */
public record PosProductConfigurationResponse(
        ProductConfiguration product, List<GroupConfiguration> groups) {

    public PosProductConfigurationResponse {
        groups = List.copyOf(groups);
    }

    public static PosProductConfigurationResponse from(Product product, List<GroupConfiguration> groups) {
        return new PosProductConfigurationResponse(
                new ProductConfiguration(
                        product.getId(),
                        product.getSku(),
                        product.getName(),
                        product.getUnitPrice(),
                        product.getCategory() == null ? null : product.getCategory().getId(),
                        product.getStatus()),
                groups);
    }

    public record ProductConfiguration(
            UUID id,
            String sku,
            String name,
            BigDecimal unitPrice,
            UUID categoryId,
            com.hiqnet.pos.catalog.domain.ProductStatus status) {
    }

    public record GroupConfiguration(
            UUID id,
            String code,
            String name,
            int minSelections,
            int maxSelections,
            ModifierGroupStatus status,
            int displayOrder,
            List<OptionConfiguration> options) {

        public GroupConfiguration {
            options = List.copyOf(options);
        }

        public static GroupConfiguration from(ModifierGroup group, int displayOrder, List<ModifierOption> options) {
            return new GroupConfiguration(
                    group.getId(),
                    group.getCode(),
                    group.getName(),
                    group.getMinSelections(),
                    group.getMaxSelections(),
                    group.getStatus(),
                    displayOrder,
                    options.stream().map(OptionConfiguration::from).toList());
        }
    }

    public record OptionConfiguration(
            UUID id,
            String code,
            String name,
            BigDecimal priceDelta,
            int displayOrder,
            ModifierOptionStatus status) {

        public static OptionConfiguration from(ModifierOption option) {
            return new OptionConfiguration(
                    option.getId(),
                    option.getCode(),
                    option.getName(),
                    option.getPriceDelta(),
                    option.getDisplayOrder(),
                    option.getStatus());
        }
    }
}
