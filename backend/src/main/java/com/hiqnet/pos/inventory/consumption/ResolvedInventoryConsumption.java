package com.hiqnet.pos.inventory.consumption;

import java.util.List;

public record ResolvedInventoryConsumption(boolean tracked, List<ResolvedInventoryComponent> components) {
    public ResolvedInventoryConsumption {
        components = List.copyOf(components);
    }

    public static ResolvedInventoryConsumption untracked() {
        return new ResolvedInventoryConsumption(false, List.of());
    }
}
