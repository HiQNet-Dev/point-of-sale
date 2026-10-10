package com.hiqnet.pos.catalog.application;

import com.hiqnet.pos.catalog.domain.ModifierGroup;

/**
 * A modifier group attached to a product, with its position in the ordered
 * configuration list.
 */
public record ProductModifierAssignment(ModifierGroup group, int displayOrder) {
}
