package com.hiqnet.pos.payment.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

public interface PaymentRepository extends JpaRepository<Payment, UUID> {
    List<Payment> findBySaleIdOrderByPositionAsc(UUID saleId);
}
