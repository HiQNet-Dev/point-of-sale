package com.hiqnet.pos.sales.domain;

/** A sale is either under construction or historically cancelled in this slice. */
public enum SaleStatus {
    OPEN,
    CANCELLED
}
