import { randomUUID } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { URL } from 'node:url'
import { writeFile } from 'node:fs/promises'
import { request } from '@playwright/test'
import type { APIRequestContext, APIResponse } from '@playwright/test'

const apiBase = process.env.E2E_API_URL ?? 'http://127.0.0.1:8080/api/v1'
const organizationCode = process.env.E2E_ORGANIZATION_CODE ?? 'pos-e2e'
const adminEmail = process.env.E2E_ADMIN_EMAIL ?? 'admin@pos-e2e.invalid'
const cashierEmail = process.env.E2E_CASHIER_EMAIL ?? 'cashier@pos-e2e.invalid'
const adminPassword = process.env.E2E_ADMIN_PASSWORD
const cashierPassword = process.env.E2E_CASHIER_PASSWORD
const bootstrapSecret = process.env.BOOTSTRAP_SECRET
const fixturePath = join(tmpdir(), 'hiqnet-pos-e2e-fixture.json')

function requireValue(name: string, value: string | undefined): string {
  if (!value) throw new Error(`Missing required E2E environment variable ${name}`)
  return value
}

async function waitForBackend() {
  const deadline = Date.now() + 120_000
  let lastError = 'backend did not respond'
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${apiBase.replace(/\/api\/v1$/, '')}/actuator/health`)
      if (response.ok) return
      lastError = `health returned HTTP ${response.status}`
    } catch (error) {
      lastError = error instanceof Error ? error.message : 'unknown network error'
    }
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  throw new Error(`Backend was not healthy after 120 seconds: ${lastError}`)
}

async function main() {
  assertSafeEnvironment()
  requireValue('BOOTSTRAP_SECRET', bootstrapSecret)
  requireValue('E2E_ADMIN_PASSWORD', adminPassword)
  requireValue('E2E_CASHIER_PASSWORD', cashierPassword)
  await waitForBackend()

  const api = await request.newContext()
  try {
    const bootstrap = await api.post(apiUrl('/bootstrap'), {
      headers: { 'X-Bootstrap-Token': bootstrapSecret! },
      data: {
        organization: {
          name: 'HiQNet POS E2E',
          code: organizationCode,
          timezone: 'America/Mexico_City',
          currency: 'MXN',
          locale: 'es-MX',
        },
        administrator: { email: adminEmail, password: adminPassword },
      },
    })
    const provisioned = await jsonResponse(bootstrap, 'secure bootstrap', [201])
    const organizationId = provisioned.organizationId as string
    const adminToken = await login(api, organizationCode, adminEmail, adminPassword!)

    const admin = (method: 'POST' | 'PUT' | 'PATCH', path: string, data: unknown) =>
      api.fetch(apiUrl(path), { method, headers: bearer(adminToken), data })

    const branchA = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches`, { name: 'E2E Branch A', code: 'e2e-a' }),
      'create branch A', [201],
    )
    const branchB = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches`, { name: 'E2E Branch B', code: 'e2e-b' }),
      'create branch B', [201],
    )
    const registerA1 = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches/${branchA.id}/cash-registers`, {
        name: 'E2E Register A1', code: 'e2e-a1',
      }), 'create register A1', [201],
    )
    await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches/${branchA.id}/cash-registers`, {
        name: 'E2E Register A2', code: 'e2e-a2',
      }), 'create register A2', [201],
    )
    await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches/${branchB.id}/cash-registers`, {
        name: 'E2E Register B1', code: 'e2e-b1',
      }), 'create register B1', [201],
    )

    const permissionsResponse = await api.get(apiUrl(`/organizations/${organizationId}/permissions`), {
      headers: bearer(adminToken),
    })
    const availablePermissions = await jsonResponse(permissionsResponse, 'list permission catalog', [200]) as Array<{ code: string }>
    const requiredCashierPermissions = [
      'cash-register.read', 'cash-shift.read', 'cash-shift.open', 'cash-shift.close',
      'pos.catalog.read', 'sale.read', 'sale.create', 'sale.update', 'sale.cancel', 'sale.checkout',
    ]
    const permissionCodes = new Set(availablePermissions.map(({ code }) => code))
    const missingPermissions = requiredCashierPermissions.filter((code) => !permissionCodes.has(code))
    if (missingPermissions.length) throw new Error(`Required cashier permissions not found: ${missingPermissions.join(', ')}`)

    const cashierRole = await createRole(admin, organizationId, 'e2e-cashier', 'E2E Cashier', requiredCashierPermissions)
    const noCatalogRole = await createRole(admin, organizationId, 'e2e-no-catalog', 'E2E No Catalog', [
      'cash-register.read', 'cash-shift.read', 'sale.read', 'sale.create', 'sale.update', 'sale.cancel', 'sale.checkout',
    ])
    const noMutationRole = await createRole(admin, organizationId, 'e2e-no-mutation', 'E2E No Mutation', [
      'cash-register.read', 'cash-shift.read', 'cash-shift.open', 'cash-shift.close',
      'pos.catalog.read', 'sale.read', 'sale.create', 'sale.cancel', 'sale.checkout',
    ])

    const cashier = await createUser(admin, organizationId, cashierEmail, cashierPassword!)
    const noCatalog = await createUser(admin, organizationId, 'no-catalog@pos-e2e.invalid', cashierPassword!)
    const noMutation = await createUser(admin, organizationId, 'no-mutation@pos-e2e.invalid', cashierPassword!)
    await assignMembership(admin, organizationId, branchA.id, cashier.id, cashierRole.id)
    await assignMembership(admin, organizationId, branchA.id, noCatalog.id, noCatalogRole.id)
    await assignMembership(admin, organizationId, branchA.id, noMutation.id, noMutationRole.id)

    const activeCategory = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/categories`, { code: 'e2e-active', name: 'E2E Active' }),
      'create active category', [201],
    )
    const inactiveCategory = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/categories`, { code: 'e2e-inactive', name: 'E2E Inactive' }),
      'create inactive category', [201],
    )
    const simple = await createProduct(admin, organizationId, {
      sku: 'E2E-SIMPLE', name: 'E2E Simple Product', unitPrice: '10.1000', categoryId: activeCategory.id,
    })
    const zero = await createProduct(admin, organizationId, {
      sku: 'E2E-ZERO', name: 'E2E Zero Product', unitPrice: '0.0000', categoryId: activeCategory.id,
    })
    const required = await createProduct(admin, organizationId, {
      sku: 'E2E-REQUIRED', name: 'E2E Required Product', unitPrice: '1.2345', categoryId: activeCategory.id,
    })
    const multiple = await createProduct(admin, organizationId, {
      sku: 'E2E-MULTI', name: 'E2E Multiple Choice Product', unitPrice: '1.2345', categoryId: activeCategory.id,
    })
    const inactiveProduct = await createProduct(admin, organizationId, {
      sku: 'E2E-INACTIVE', name: 'E2E Inactive Product', unitPrice: '1.0000', categoryId: activeCategory.id,
    })
    const inactiveCategoryProduct = await createProduct(admin, organizationId, {
      sku: 'E2E-INACTIVE-CAT', name: 'E2E Active Under Inactive Category', unitPrice: '0.2000', categoryId: inactiveCategory.id,
    })

    const beans = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/inventory-items`, {
        code: 'e2e-beans', name: 'E2E Beans', baseUnit: 'GRAM', fractionalUnitAllowed: false,
      }),
      'create inventory item', [201],
    )
    const tracked = await createProduct(admin, organizationId, {
      sku: 'E2E-TRACKED', name: 'E2E Tracked Product', unitPrice: '12.3400', categoryId: activeCategory.id,
    })
    await jsonResponse(
      await admin('PUT', `/organizations/${organizationId}/products/${tracked.id}/inventory-recipe`, {
        expectedVersion: 0, status: 'ACTIVE', components: [{ inventoryItemId: beans.id, quantity: '25' }],
      }), 'create tracked product recipe', [200],
    )
    await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/branches/${branchA.id}/inventory-items/${beans.id}/opening-stock`, {
        expectedVersion: 0, quantity: '1000',
      }), 'open branch A inventory stock', [200],
    )

    const requiredGroup = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups`, {
        code: 'e2e-required', name: 'Required choice', minSelections: 1, maxSelections: 1,
      }), 'create required modifier group', [201],
    )
    const option = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups/${requiredGroup.id}/options`, {
        code: 'e2e-required-option', name: 'Required option', priceDelta: '0.2000', displayOrder: 0,
      }), 'create required modifier option', [201],
    )
    const multipleGroup = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups`, {
        code: 'e2e-multiple', name: 'Multiple choice', minSelections: 0, maxSelections: 2,
      }), 'create multiple-choice group', [201],
    )
    const activeOptionOne = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups/${multipleGroup.id}/options`, {
        code: 'e2e-multi-one', name: 'Positive option', priceDelta: '0.2000', displayOrder: 0,
      }), 'create positive option', [201],
    )
    await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups/${multipleGroup.id}/options`, {
        code: 'e2e-multi-two', name: 'Negative option', priceDelta: '-0.1000', displayOrder: 1,
      }), 'create negative option', [201],
    )
    const inactiveOption = await jsonResponse(
      await admin('POST', `/organizations/${organizationId}/modifier-groups/${multipleGroup.id}/options`, {
        code: 'e2e-multi-inactive', name: 'Inactive option', priceDelta: '0.0000', displayOrder: 2,
      }), 'create inactive option', [201],
    )
    await jsonResponse(
      await admin('PATCH', `/organizations/${organizationId}/modifier-groups/${multipleGroup.id}/options/${inactiveOption.id}`, {
        expectedVersion: inactiveOption.version, status: 'INACTIVE',
      }), 'deactivate one modifier option', [200],
    )
    await jsonResponse(
      await admin('PUT', `/organizations/${organizationId}/products/${required.id}/modifier-groups`, {
        expectedVersion: required.version, modifierGroupIds: [requiredGroup.id],
      }), 'attach required modifier group', [200],
    )
    await jsonResponse(
      await admin('PUT', `/organizations/${organizationId}/products/${multiple.id}/modifier-groups`, {
        expectedVersion: multiple.version, modifierGroupIds: [multipleGroup.id],
      }), 'attach multiple-choice group', [200],
    )
    await jsonResponse(
      await admin('PATCH', `/organizations/${organizationId}/products/${inactiveProduct.id}`, {
        expectedVersion: inactiveProduct.version, status: 'INACTIVE',
      }), 'deactivate inactive Product fixture', [200],
    )
    await jsonResponse(
      await admin('PATCH', `/organizations/${organizationId}/categories/${inactiveCategory.id}`, {
        expectedVersion: inactiveCategory.version, status: 'INACTIVE',
      }), 'deactivate category after product creation', [200],
    )

    await writeFile(fixturePath, JSON.stringify({
      organizationId,
      organizationCode,
      branchAId: branchA.id,
      branchBId: branchB.id,
      registerA1Id: registerA1.id,
      cashierEmail,
      noCatalogEmail: 'no-catalog@pos-e2e.invalid',
      noMutationEmail: 'no-mutation@pos-e2e.invalid',
      simpleProductId: simple.id,
      simpleProductName: simple.name,
      trackedProductId: tracked.id,
      trackedProductName: tracked.name,
      trackedItemId: beans.id,
      zeroProductId: zero.id,
      zeroProductName: zero.name,
      requiredProductId: required.id,
      requiredProductName: required.name,
      requiredOptionName: option.name,
      multipleProductName: multiple.name,
      activeOptionOneName: activeOptionOne.name,
      activeOptionTwoName: 'Negative option',
      inactiveOptionName: 'Inactive option',
      inactiveCategoryProductName: inactiveCategoryProduct.name,
    }), { mode: 0o600 })
  } finally {
    await api.dispose()
  }
}

function assertSafeEnvironment() {
  if (process.env.APP_ENV !== 'e2e') throw new Error('Refusing E2E setup unless APP_ENV=e2e.')
  const databaseUrl = requireValue('E2E_DB_URL', process.env.E2E_DB_URL)
  const jdbc = databaseUrl.match(/^jdbc:postgresql:\/\/([^/]+)\/([^?]+)(?:\?.*)?$/)
  if (!jdbc) throw new Error('E2E_DB_URL must name a PostgreSQL database explicitly.')
  const databaseHost = new URL(`http://${jdbc[1]}`).hostname
  const localHosts = new Set(['localhost', '127.0.0.1', '::1', '[::1]'])
  if (!localHosts.has(databaseHost) || !decodeURIComponent(jdbc[2]).startsWith('hiqnet_pos_e2e')) {
    throw new Error('Refusing E2E setup unless the database is local and named hiqnet_pos_e2e*.')
  }
  const apiHost = new URL(apiBase).hostname
  if (!localHosts.has(apiHost)) throw new Error('Refusing to provision fixtures through a non-local API host.')
}

function apiUrl(path: string) {
  return `${apiBase}${path}`
}

function bearer(token: string) {
  return { Authorization: `Bearer ${token}` }
}

async function jsonResponse(response: APIResponse, label: string, expected: number[]) {
  if (!expected.includes(response.status())) {
    const body = await response.text()
    throw new Error(`${label} failed with HTTP ${response.status()}: ${body.slice(0, 1000)}`)
  }
  return response.status() === 204 ? {} : response.json()
}

async function login(api: APIRequestContext, code: string, email: string, password: string) {
  const response = await api.post(apiUrl('/auth/login'), { data: { organizationCode: code, email, password } })
  const body = await jsonResponse(response, 'login admin', [200]) as { accessToken: string }
  return body.accessToken
}

async function createRole(
  admin: (method: 'POST' | 'PUT' | 'PATCH', path: string, data: unknown) => Promise<APIResponse>,
  organizationId: string,
  code: string,
  name: string,
  permissions: string[],
) {
  const role = await jsonResponse(await admin('POST', `/organizations/${organizationId}/roles`, { code, name }), `create role ${code}`, [201])
  await jsonResponse(
    await admin('PUT', `/organizations/${organizationId}/roles/${role.id}/permissions`, { permissions }),
    `grant permissions to ${code}`, [200],
  )
  return role
}

async function createUser(
  admin: (method: 'POST' | 'PUT' | 'PATCH', path: string, data: unknown) => Promise<APIResponse>,
  organizationId: string,
  email: string,
  password: string,
) {
  return jsonResponse(await admin('POST', `/organizations/${organizationId}/users`, { email, password }), `create user ${email}`, [201])
}

async function assignMembership(
  admin: (method: 'POST' | 'PUT' | 'PATCH', path: string, data: unknown) => Promise<APIResponse>,
  organizationId: string,
  branchId: string,
  userId: string,
  roleId: string,
) {
  await jsonResponse(
    await admin('POST', `/organizations/${organizationId}/branches/${branchId}/memberships`, { userId }),
    `create membership for ${userId}`, [201],
  )
  await jsonResponse(
    await admin('PUT', `/organizations/${organizationId}/branches/${branchId}/memberships/${userId}/roles`, { roleIds: [roleId] }),
    `assign branch role to ${userId}`, [200],
  )
}

async function createProduct(
  admin: (method: 'POST' | 'PUT' | 'PATCH', path: string, data: unknown) => Promise<APIResponse>,
  organizationId: string,
  product: { sku: string; name: string; unitPrice: string; categoryId: string },
) {
  return jsonResponse(
    await admin('POST', `/organizations/${organizationId}/products`, {
      ...product,
      barcode: randomUUID().replaceAll('-', '').slice(0, 12),
    }),
    `create product ${product.sku}`, [201],
  )
}

export default main
