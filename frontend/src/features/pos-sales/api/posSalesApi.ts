import { requestJson } from '../../../shared/api/client'
import type { PageResponse } from '../../pos-session/api/posSessionApi'

export type SaleStatus = 'OPEN' | 'CANCELLED' | 'COMPLETED'
export type ItemStatus = 'ACTIVE' | 'REMOVED'
export type CatalogStatus = 'ACTIVE' | 'INACTIVE'
export type PaymentMethod = 'CASH' | 'CARD'
export type ExactAmount = string

const SALE_SUMMARY_DECIMAL_FIELDS = ['totalAmount'] as const
const SALE_DETAIL_DECIMAL_FIELDS = [
  ...SALE_SUMMARY_DECIMAL_FIELDS,
  'baseUnitPrice',
  'modifierUnitDelta',
  'effectiveUnitPrice',
  'lineTotal',
  'priceDeltaSnapshot',
] as const

export interface SaleSummary {
  id: string
  organizationId: string
  branchId: string
  cashRegisterId: string
  cashShiftId: string
  status: SaleStatus
  totalAmount: ExactAmount
  version: number
  createdAt: string
  completedAt: string | null
  cancelledAt: string | null
}

export interface SaleItemModifier {
  id: string
  modifierGroupId: string
  modifierOptionId: string
  groupNameSnapshot: string
  optionNameSnapshot: string
  priceDeltaSnapshot: ExactAmount
  displayOrder: number
}

export interface SaleItem {
  id: string
  productId: string
  productSkuSnapshot: string
  productNameSnapshot: string
  baseUnitPrice: ExactAmount
  modifierUnitDelta: ExactAmount
  effectiveUnitPrice: ExactAmount
  quantity: number
  lineTotal: ExactAmount
  status: ItemStatus
  position: number
  modifiers: SaleItemModifier[]
}

export interface SaleDetail {
  sale: SaleSummary
  items: SaleItem[]
}

export interface Category {
  id: string
  code: string
  name: string
  status: CatalogStatus
}

export interface Product {
  id: string
  sku: string
  barcode: string | null
  name: string
  description: string | null
  unitPrice: number
  categoryId: string | null
  status: 'ACTIVE'
}

export interface ProductOption {
  id: string
  code: string
  name: string
  priceDelta: number
  displayOrder: number
  status: CatalogStatus
}

export interface ModifierGroup {
  id: string
  code: string
  name: string
  minSelections: number
  maxSelections: number
  status: CatalogStatus
  displayOrder: number
  options: ProductOption[]
}

export interface ProductConfiguration {
  product: Pick<Product, 'id' | 'sku' | 'name' | 'unitPrice' | 'categoryId' | 'status'>
  groups: ModifierGroup[]
}

const PAGE_SIZE = 20
const CATEGORY_PAGE_SIZE = 100

function segment(value: string): string {
  return encodeURIComponent(value)
}

function salesPath(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
): string {
  return `/organizations/${segment(organizationId)}/branches/${segment(branchId)}`
    + `/cash-registers/${segment(cashRegisterId)}/cash-shifts/${segment(cashShiftId)}/sales`
}

function operationalCatalogPath(organizationId: string, branchId: string): string {
  return `/organizations/${segment(organizationId)}/branches/${segment(branchId)}/pos/catalog`
}

export function listOpenSales(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  page = 0,
): Promise<PageResponse<SaleSummary>> {
  const params = new URLSearchParams({ page: String(page), size: String(PAGE_SIZE), status: 'OPEN' })
  return requestJson(`${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}?${params}`, {}, {
    exactDecimalFields: SALE_SUMMARY_DECIMAL_FIELDS,
  })
}

export function createSale(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
): Promise<SaleSummary> {
  return requestJson(salesPath(organizationId, branchId, cashRegisterId, cashShiftId), { method: 'POST' }, {
    exactDecimalFields: SALE_SUMMARY_DECIMAL_FIELDS,
  })
}

export function getSale(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
): Promise<SaleDetail> {
  return requestJson(`${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}`, {}, {
    exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS,
  })
}

export function checkoutSale(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
  input: {
    expectedSaleVersion: number
    payments: Array<{ method: PaymentMethod; amount: ExactAmount; tenderedAmount?: ExactAmount }>
  },
): Promise<SaleDetail> {
  return requestJson(
    `${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}/checkout`,
    { method: 'POST', body: JSON.stringify(input) },
    { exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS },
  )
}

export function addSaleItem(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
  input: { expectedSaleVersion: number; productId: string; quantity: number; modifierOptionIds: string[] },
): Promise<SaleDetail> {
  return requestJson(`${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}/items`, {
    method: 'POST',
    body: JSON.stringify(input),
  }, { exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS })
}

export function updateSaleItem(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
  saleItemId: string,
  input: { expectedSaleVersion: number; quantity: number; modifierOptionIds: string[] },
): Promise<SaleDetail> {
  return requestJson(
    `${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}/items/${segment(saleItemId)}`,
    { method: 'PATCH', body: JSON.stringify(input) },
    { exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS },
  )
}

export function removeSaleItem(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
  saleItemId: string,
  expectedVersion: number,
): Promise<SaleDetail> {
  return requestJson(
    `${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}/items/${segment(saleItemId)}/remove`,
    { method: 'POST', body: JSON.stringify({ expectedVersion }) },
    { exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS },
  )
}

export function cancelSale(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  saleId: string,
  expectedVersion: number,
): Promise<SaleDetail> {
  return requestJson(
    `${salesPath(organizationId, branchId, cashRegisterId, cashShiftId)}/${segment(saleId)}/cancel`,
    { method: 'POST', body: JSON.stringify({ expectedVersion }) },
    { exactDecimalFields: SALE_DETAIL_DECIMAL_FIELDS },
  )
}

export function listOperationalCategories(
  organizationId: string,
  branchId: string,
  page: number,
): Promise<PageResponse<Category>> {
  const params = new URLSearchParams({ page: String(page), size: String(CATEGORY_PAGE_SIZE) })
  return requestJson(`${operationalCatalogPath(organizationId, branchId)}/categories?${params}`)
}

export function listOperationalProducts(
  organizationId: string,
  branchId: string,
  input: { page: number; q: string; categoryId: string | null },
): Promise<PageResponse<Product>> {
  const params = new URLSearchParams({ page: String(input.page), size: String(PAGE_SIZE) })
  if (input.q.trim()) params.set('q', input.q.trim())
  if (input.categoryId) params.set('categoryId', input.categoryId)
  return requestJson(`${operationalCatalogPath(organizationId, branchId)}/products?${params}`)
}

export function getProductConfiguration(
  organizationId: string,
  branchId: string,
  productId: string,
): Promise<ProductConfiguration> {
  return requestJson(
    `${operationalCatalogPath(organizationId, branchId)}/products/${segment(productId)}/configuration`,
  )
}

export const posSalesPageSize = PAGE_SIZE
export const posCategoryPageSize = CATEGORY_PAGE_SIZE
