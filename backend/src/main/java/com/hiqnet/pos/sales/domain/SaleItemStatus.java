package com.hiqnet.pos.sales.domain;

/** Sale lines are retained for history; removal never physically deletes a line. */
public enum SaleItemStatus {
    ACTIVE,
    REMOVED
}
