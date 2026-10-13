import type { ProductListQuery } from '../api/inventoryRecipesApi'

export const inventoryRecipesKeys = {
  all: ['inventory', 'recipes'] as const,
  products: (organizationId: string, query: ProductListQuery) =>
    [...inventoryRecipesKeys.all, 'products', organizationId, query] as const,
  categories: (organizationId: string) =>
    [...inventoryRecipesKeys.all, 'categories', organizationId] as const,
  recipe: (organizationId: string, productId: string) =>
    [...inventoryRecipesKeys.all, 'recipe', organizationId, productId] as const,
}
