import { randomUUID } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, request as playwrightRequest } from '@playwright/test'
import type { APIRequestContext, APIResponse, Page } from '@playwright/test'

/**
 * Utilidades compartidas por los specs E2E de Inventory. Reutiliza el harness
 * existente (global-setup + API real) y no introduce un segundo framework.
 */

export const API_BASE = process.env.E2E_API_URL ?? 'http://127.0.0.1:8080/api/v1'
export const ORGANIZATION_CODE = process.env.E2E_ORGANIZATION_CODE ?? 'pos-e2e'
export const ADMIN_EMAIL = process.env.E2E_ADMIN_EMAIL ?? 'admin@pos-e2e.invalid'
export const ADMIN_PASSWORD = process.env.E2E_ADMIN_PASSWORD ?? ''
export const CASHIER_EMAIL = process.env.E2E_CASHIER_EMAIL ?? 'cashier@pos-e2e.invalid'
export const CASHIER_PASSWORD = process.env.E2E_CASHIER_PASSWORD ?? ''

const FIXTURE_FILE = join(tmpdir(), 'hiqnet-pos-e2e-fixture.json')

export interface GlobalFixtures {
  organizationId: string
  organizationCode: string
  branchAId: string
  branchBId: string
  registerA1Id: string
  trackedItemId: string
}

/** Lee el fixture global que `global-setup.ts` escribe para cada ejecución E2E. */
export async function readGlobalFixtures(): Promise<GlobalFixtures> {
  return JSON.parse(await readFile(FIXTURE_FILE, 'utf8')) as GlobalFixtures
}

/** Completa el formulario de login; asume que la pantalla ya está visible. */
export async function signIn(page: Page, email: string, password: string): Promise<void> {
  await page.getByLabel('Organization code').fill(ORGANIZATION_CODE)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Sign in' }).click()
}

/**
 * Abre una ruta protegida, resuelve el redirect a `/login` e inicia sesión.
 * El auth es memory-only, así que las navegaciones posteriores deben ser in-app.
 */
export async function signInAt(page: Page, path: string, email: string, password: string): Promise<void> {
  await page.goto(path)
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible()
  await signIn(page, email, password)
}

export type InventoryBaseUnit = 'UNIT' | 'GRAM' | 'MILLILITER'
export type InventoryNegativeStockPolicy = 'STRICT' | 'ALLOW_NEGATIVE'

export interface InventoryItemRow {
  id: string
  code: string
  name: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: 'ACTIVE' | 'INACTIVE'
  version: number
}

export interface ProductRow {
  id: string
  sku: string
  name: string
  version: number
}

export interface StockRow {
  inventoryItemId: string
  code: string
  name: string
  quantityOnHand: string
  baseUnit: InventoryBaseUnit
  status: 'ACTIVE' | 'INACTIVE'
  version: number
}

export interface MovementRow {
  id: string
  type: 'OPENING' | 'ADJUSTMENT' | 'SALE_CONSUMPTION'
  quantityDelta: string
  resultingQuantity: string
  reason: string | null
  saleId: string | null
  occurredAt: string
}

export interface OrganizationRow {
  id: string
  version: number
  inventoryNegativeStockPolicy: InventoryNegativeStockPolicy
}

export interface BranchRow {
  id: string
  name: string
  code: string
}

export interface RecipeRow {
  id: string
  productId: string
  status: 'ACTIVE' | 'INACTIVE'
  version: number
  components: Array<{ inventoryItemId: string; code: string; quantity: string }>
}

export interface EffectsRow {
  id: string
  modifierOptionId: string
  version: number
  effects: Array<{ inventoryItemId: string; code: string; quantityDelta: string }>
}

export interface SaleStatusRow {
  sale: { id: string; status: string }
}

export interface AdminApi {
  readonly organizationId: string
  dispose(): Promise<void>
  createCategory(code: string, name: string): Promise<{ id: string; version: number }>
  createInventoryItem(input: {
    code: string
    name: string
    baseUnit: InventoryBaseUnit
    fractionalUnitAllowed?: boolean
  }): Promise<InventoryItemRow>
  createProduct(input: { sku: string; name: string; unitPrice: string; categoryId?: string }): Promise<ProductRow>
  setRecipe(
    productId: string,
    input: { expectedVersion: number; status: 'ACTIVE' | 'INACTIVE'; components: Array<{ inventoryItemId: string; quantity: string }> },
  ): Promise<RecipeRow>
  createModifierGroup(input: { code: string; name: string; minSelections: number; maxSelections: number }): Promise<{ id: string }>
  createModifierOption(
    groupId: string,
    input: { code: string; name: string; priceDelta: string; displayOrder: number },
  ): Promise<{ id: string }>
  setModifierEffects(
    optionId: string,
    input: { expectedVersion: number; effects: Array<{ inventoryItemId: string; quantityDelta: string }> },
  ): Promise<EffectsRow>
  createCashRegister(branchId: string, input: { name: string; code: string }): Promise<{ id: string }>
  openStock(
    branchId: string,
    itemId: string,
    input: { expectedVersion: number; quantity: string; reason?: string },
  ): Promise<StockRow>
  adjustStock(
    branchId: string,
    itemId: string,
    input: { expectedVersion: number; quantityDelta: string; reason: string },
  ): Promise<{ quantityOnHand: string; version: number }>
  getStock(branchId: string, itemId: string): Promise<StockRow>
  listMovements(branchId: string, itemId: string): Promise<MovementRow[]>
  findInventoryItemByCode(code: string): Promise<InventoryItemRow | undefined>
  getRecipe(productId: string): Promise<RecipeRow>
  getModifierEffects(optionId: string): Promise<EffectsRow>
  getOrganization(): Promise<OrganizationRow>
  setNegativeStockPolicy(
    expectedVersion: number,
    policy: InventoryNegativeStockPolicy,
  ): Promise<OrganizationRow>
  listBranches(): Promise<BranchRow[]>
  getSaleStatus(input: {
    branchId: string
    registerId: string
    shiftId: string
    saleId: string
  }): Promise<string>
}

async function login(
  context: APIRequestContext,
  organizationCode: string,
  email: string,
  password: string,
): Promise<string> {
  const response = await context.post(`${API_BASE}/auth/login`, { data: { organizationCode, email, password } })
  const body = await expectJson<{ accessToken: string }>(response, 'login admin', [200])
  return body.accessToken
}

async function expectJson<T>(response: APIResponse, label: string, expected: number[]): Promise<T> {
  if (!expected.includes(response.status())) {
    throw new Error(`${label} falló con HTTP ${response.status()}: ${(await response.text()).slice(0, 600)}`)
  }
  if (response.status() === 204) return undefined as T
  return (await response.json()) as T
}

/** Cliente API con el rol administrator (todos los permisos) para ARRANGE y verificación. */
export async function createAdminApi(): Promise<AdminApi> {
  const fixtures = await readGlobalFixtures()
  const context = await playwrightRequest.newContext()
  const token = await login(context, ORGANIZATION_CODE, ADMIN_EMAIL, ADMIN_PASSWORD)
  const organizationId = fixtures.organizationId
  const auth = { Authorization: `Bearer ${token}` }

  const apiUrl = (suffix: string) => `${API_BASE}${suffix}`
  const org = (suffix: string) => apiUrl(`/organizations/${organizationId}${suffix}`)

  async function post<T>(label: string, path: string, data: unknown, expected: number[]): Promise<T> {
    return expectJson<T>(await context.post(path, { headers: auth, data }), label, expected)
  }

  async function openStockAndReturn(
    branchId: string,
    itemId: string,
    input: { expectedVersion: number; quantity: string; reason?: string },
  ): Promise<StockRow> {
    await post<StockRow>(
      'opening stock',
      org(`/branches/${branchId}/inventory-items/${itemId}/opening-stock`),
      input,
      [200],
    )
    return getStock(branchId, itemId)
  }

  async function getStock(branchId: string, itemId: string): Promise<StockRow> {
    return expectJson<StockRow>(
      await context.get(org(`/branches/${branchId}/inventory-items/${itemId}`), { headers: auth }),
      'get stock',
      [200],
    )
  }

  return {
    organizationId,
    async dispose() {
      await context.dispose()
    },
    async createCategory(code, name) {
      return post('create category', org('/categories'), { code, name }, [201])
    },
    async createInventoryItem(input) {
      return post('create inventory item', org('/inventory-items'), input, [201])
    },
    async createProduct(input) {
      return post(
        'create product',
        org('/products'),
        { ...input, barcode: randomUUID().replaceAll('-', '').slice(0, 12) },
        [201],
      )
    },
    async setRecipe(productId, input) {
      return expectJson<RecipeRow>(
        await context.put(org(`/products/${productId}/inventory-recipe`), { headers: auth, data: input }),
        'replace recipe',
        [200],
      )
    },
    async createModifierGroup(input) {
      return post('create modifier group', org('/modifier-groups'), input, [201])
    },
    async createModifierOption(groupId, input) {
      return post('create modifier option', org(`/modifier-groups/${groupId}/options`), input, [201])
    },
    async setModifierEffects(optionId, input) {
      return expectJson<EffectsRow>(
        await context.put(org(`/modifier-options/${optionId}/inventory-effects`), { headers: auth, data: input }),
        'replace modifier effects',
        [200],
      )
    },
    async createCashRegister(branchId, input) {
      return post('create cash register', org(`/branches/${branchId}/cash-registers`), input, [201])
    },
    async openStock(branchId, itemId, input) {
      return openStockAndReturn(branchId, itemId, input)
    },
    async adjustStock(branchId, itemId, input) {
      return post<{ quantityOnHand: string; version: number }>(
        'adjust stock',
        org(`/branches/${branchId}/inventory-items/${itemId}/adjustments`),
        input,
        [200],
      )
    },
    getStock,
    async listMovements(branchId, itemId) {
      const page = await expectJson<{ items: MovementRow[] }>(
        await context.get(org(`/branches/${branchId}/inventory-items/${itemId}/movements`), { headers: auth }),
        'list movements',
        [200],
      )
      return page.items
    },
    async findInventoryItemByCode(code) {
      const page = await expectJson<{ items: InventoryItemRow[] }>(
        await context.get(org(`/inventory-items?page=0&size=100&q=${encodeURIComponent(code)}`), { headers: auth }),
        'find inventory item',
        [200],
      )
      return page.items.find((item) => item.code === code)
    },
    async getRecipe(productId) {
      return expectJson<RecipeRow>(
        await context.get(org(`/products/${productId}/inventory-recipe`), { headers: auth }),
        'get recipe',
        [200],
      )
    },
    async getModifierEffects(optionId) {
      return expectJson<EffectsRow>(
        await context.get(org(`/modifier-options/${optionId}/inventory-effects`), { headers: auth }),
        'get modifier effects',
        [200],
      )
    },
    async getOrganization() {
      return expectJson<OrganizationRow>(
        await context.get(org(''), { headers: auth }),
        'get organization',
        [200],
      )
    },
    async setNegativeStockPolicy(expectedVersion, policy) {
      return expectJson<OrganizationRow>(
        await context.patch(org(''), {
          headers: auth,
          data: { expectedVersion, inventoryNegativeStockPolicy: policy },
        }),
        'patch organization policy',
        [200],
      )
    },
    async listBranches() {
      const page = await expectJson<{ items: BranchRow[] }>(
        await context.get(apiUrl('/auth/me/branches?page=0&size=100'), { headers: auth }),
        'list branches',
        [200],
      )
      return page.items
    },
    async getSaleStatus(input) {
      const detail = await expectJson<SaleStatusRow>(
        await context.get(
          org(
            `/branches/${input.branchId}/cash-registers/${input.registerId}` +
              `/cash-shifts/${input.shiftId}/sales/${input.saleId}`,
          ),
          { headers: auth },
        ),
        'get sale detail',
        [200],
      )
      return detail.sale.status
    },
  }
}
