package com.hiqnet.pos.sales.application;

import com.hiqnet.pos.cashshift.application.OpenSalesForCashShiftPort;
import com.hiqnet.pos.sales.domain.SaleRepository;
import com.hiqnet.pos.sales.domain.SaleStatus;
import java.util.UUID;
import org.springframework.stereotype.Component;

/** Adapter that keeps the CashShift -> Sales dependency pointed at a port. */
@Component
public class OpenSalesForCashShiftAdapter implements OpenSalesForCashShiftPort {
    private final SaleRepository saleRepository;
    public OpenSalesForCashShiftAdapter(SaleRepository saleRepository) { this.saleRepository = saleRepository; }
    @Override public boolean hasOpenSales(UUID cashShiftId) {
        return saleRepository.existsByCashShiftIdAndStatus(cashShiftId, SaleStatus.OPEN);
    }
}
