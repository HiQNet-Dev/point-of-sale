package com.hiqnet.pos.sales.api;

import com.hiqnet.pos.sales.application.SaleService.SaleDetail;
import java.util.List;

public record SaleDetailResponse(SaleResponse sale, List<SaleItemResponse> items) {
    public static SaleDetailResponse from(SaleDetail detail) {
        return new SaleDetailResponse(SaleResponse.from(detail.sale()), detail.items().stream()
                .map(item -> SaleItemResponse.from(item, detail.modifiersByItem().getOrDefault(item.getId(), List.of())))
                .toList());
    }
}
