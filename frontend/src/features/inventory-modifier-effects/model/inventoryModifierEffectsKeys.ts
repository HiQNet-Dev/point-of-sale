import type { ModifierGroupListQuery } from '../api/inventoryModifierEffectsApi'

export const inventoryModifierEffectsKeys = {
  all: ['inventory', 'modifier-effects'] as const,
  groups: (organizationId: string, query: ModifierGroupListQuery) =>
    [...inventoryModifierEffectsKeys.all, 'groups', organizationId, query] as const,
  options: (organizationId: string, modifierGroupId: string) =>
    [...inventoryModifierEffectsKeys.all, 'options', organizationId, modifierGroupId] as const,
  effects: (organizationId: string, modifierOptionId: string) =>
    [...inventoryModifierEffectsKeys.all, 'effects', organizationId, modifierOptionId] as const,
}
