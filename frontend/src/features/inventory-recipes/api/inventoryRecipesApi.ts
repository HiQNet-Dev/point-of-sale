import { requestJson } from '../../../shared/api/client'
import { pagingParams, segment } from '../../../shared/api/pagination'
import type { ListQuery, PageResponse } from '../../../shared/api/pagination'
import type { InventoryBaseUnit, InventoryItemStatus } from '../../inventory-items/api/inventoryItemsApi'

export const PRODUCT_STATUSES = ['ACTIVE', 'INACTIVE'] as const
export type ProductStatus = (typeof PRODUCT_STATUSES)[number]

export const PRODUCT_STATUS_LABELS: Record<ProductStatus, string> = {
  ACTIVE: 'Activo',
  INACTIVE: 'Inactivo',
}

export interface Product {
  id: string
  organizationId: string
  categoryId: string | null
  sku: string
  name: string
  status: ProductStatus
  version: number
}

export interface Category {
  id: string
  organizationId: string
  code: string
  name: string
  status: 'ACTIVE' | 'INACTIVE'
}

export interface ProductListQuery extends ListQuery {
  status?: ProductStatus
  categoryId?: string
}

export const PRODUCTS_PAGE_SIZE = 20

export function listProducts(organizationId: string, query: ProductListQuery): Promise<PageResponse<Product>> {
  const params = pagingParams(query, { status: query.status, categoryId: query.categoryId })
  return requestJson(`/organizations/${segment(organizationId)}/products?${params}`)
}

export function listCategories(organizationId: string): Promise<PageResponse<Category>> {
  const params = pagingParams({ page: 0, size: 100 })
  return requestJson(`/organizations/${segment(organizationId)}/categories?${params}`)
}

export const RECIPE_STATUSES = ['ACTIVE', 'INACTIVE'] as const
export type RecipeStatus = (typeof RECIPE_STATUSES)[number]

export const RECIPE_STATUS_LABELS: Record<RecipeStatus, string> = {
  ACTIVE: 'Activa',
  INACTIVE: 'Inactiva',
}

export interface RecipeComponent {
  inventoryItemId: string
  code: string
  name: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: InventoryItemStatus
  quantity: string
}

export interface ProductRecipe {
  id: string
  organizationId: string
  productId: string
  status: RecipeStatus
  version: number
  createdAt: string
  updatedAt: string
  components: RecipeComponent[]
}

export interface RecipeComponentInput {
  inventoryItemId: string
  quantity: string
}

export interface ReplaceProductRecipeInput {
  expectedVersion: number
  status: RecipeStatus
  components: RecipeComponentInput[]
}

function recipePath(organizationId: string, productId: string): string {
  return `/organizations/${segment(organizationId)}/products/${segment(productId)}/inventory-recipe`
}

export function getProductRecipe(organizationId: string, productId: string): Promise<ProductRecipe> {
  return requestJson(recipePath(organizationId, productId))
}

export function replaceProductRecipe(
  organizationId: string,
  productId: string,
  input: ReplaceProductRecipeInput,
): Promise<ProductRecipe> {
  return requestJson(recipePath(organizationId, productId), {
    method: 'PUT',
    body: JSON.stringify(input),
  })
}
