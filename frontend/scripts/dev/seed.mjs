// Deterministic DEVELOPMENT fixture seeder for HiQNet POS.
//
// It provisions the first Organization and all demo data through the real
// public APIs (Secure Admin Bootstrap + organization administration), never
// with SQL and never through a Flyway migration. This reuses the same
// mechanism the E2E harness uses, but with a development-only safety guard and
// an explicitly-fictitious dataset.
//
// The seed is NOT idempotent: the one-shot bootstrap only succeeds on a fresh
// database. If the database is already provisioned the script refuses and asks
// for `pnpm dev:reset` (reset-then-deterministic-seed).
import { randomUUID } from 'node:crypto'
import {
  assertDevDatabaseName,
  assertLocalHost,
  fail,
  handleTopLevel,
  requireDevelopment,
} from './dev-guard.mjs'

const apiBase = process.env.DEV_API_URL ?? 'http://localhost:8080/api/v1'
const organizationCode = process.env.DEV_ORGANIZATION_CODE ?? 'pos-dev'
const organizationName = process.env.DEV_ORGANIZATION_NAME ?? 'HiQNet POS Development'
const adminEmail = process.env.DEV_ADMIN_EMAIL ?? 'admin@hiqnet.local'
const adminPassword = process.env.DEV_ADMIN_PASSWORD ?? 'admin-dev-only'
const managerEmail = process.env.DEV_MANAGER_EMAIL ?? 'manager@hiqnet.local'
const cashierEmail = process.env.DEV_CASHIER_EMAIL ?? 'cashier@hiqnet.local'
const staffPassword = process.env.DEV_STAFF_PASSWORD ?? 'staff-dev-only'
const bootstrapSecret = process.env.BOOTSTRAP_SECRET

// Permissions the cashier role needs to operate the POS (kept identical to the
// E2E cashier role; deliberately omits payment.read).
const cashierPermissions = [
  'cash-register.read',
  'cash-shift.read',
  'cash-shift.open',
  'cash-shift.close',
  'pos.catalog.read',
  'sale.read',
  'sale.create',
  'sale.update',
  'sale.cancel',
  'sale.checkout',
]

// Real (non-superuser) permissions for the inventory administration UI.
const inventoryAdminPermissions = [
  'inventory.item.read',
  'inventory.item.write',
  'inventory.stock.read',
  'inventory.stock.adjust',
  'inventory.recipe.read',
  'inventory.recipe.write',
  'category.read',
  'product.read',
  'modifier.group.read',
  'modifier.option.read',
  'branch.read',
  'pos.catalog.read',
]

async function request(method, path, { token, data, headers } = {}) {
  return fetch(`${apiBase}${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...headers,
    },
    body: data === undefined ? undefined : JSON.stringify(data),
  })
}

async function expect(res, label, statuses) {
  if (!statuses.includes(res.status)) {
    const text = await res.text().catch(() => '')
    fail(`${label} failed with HTTP ${res.status}: ${text.slice(0, 500)}`)
  }
  return res.status === 204 ? {} : res.json().catch(() => ({}))
}

async function waitForBackend() {
  const healthUrl = `${apiBase.replace(/\/api\/v1$/, '')}/actuator/health`
  const deadline = Date.now() + 120_000
  let last = 'no response'
  while (Date.now() < deadline) {
    try {
      const res = await fetch(healthUrl)
      if (res.ok) return
      last = `HTTP ${res.status}`
    } catch (error) {
      last = error instanceof Error ? error.message : 'network error'
    }
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  fail(`Backend was not healthy after 120s at ${healthUrl} (${last}).`)
}

function admin(organizationId, token) {
  const base = `/organizations/${organizationId}`
  return (method, path, data) => request(method, `${base}${path}`, { token, data })
}

async function createRole(adminCall, code, name, permissions) {
  const role = await expect(await adminCall('POST', '/roles', { code, name }), `create role ${code}`, [201])
  await expect(
    await adminCall('PUT', `/roles/${role.id}/permissions`, { permissions }),
    `grant permissions to ${code}`,
    [200],
  )
  return role
}

async function createUser(adminCall, email, password) {
  return expect(await adminCall('POST', '/users', { email, password }), `create user ${email}`, [201])
}

async function assignMembership(adminCall, branchId, userId, roleId) {
  await addMembership(adminCall, branchId, userId)
  await expect(
    await adminCall('PUT', `/branches/${branchId}/memberships/${userId}/roles`, { roleIds: [roleId] }),
    `assign role to ${userId}`,
    [200],
  )
}

async function addMembership(adminCall, branchId, userId) {
  await expect(
    await adminCall('POST', `/branches/${branchId}/memberships`, { userId }),
    `create membership ${userId}`,
    [201],
  )
}

async function createProduct(adminCall, { sku, name, unitPrice, categoryId }) {
  return expect(
    await adminCall('POST', '/products', {
      sku,
      name,
      unitPrice,
      categoryId,
      barcode: randomUUID().replaceAll('-', '').slice(0, 12),
    }),
    `create product ${sku}`,
    [201],
  )
}

async function createItem(adminCall, { code, name, baseUnit, fractionalUnitAllowed }) {
  return expect(
    await adminCall('POST', '/inventory-items', { code, name, baseUnit, fractionalUnitAllowed }),
    `create item ${code}`,
    [201],
  )
}

async function openStock(adminCall, branchId, item, quantity) {
  await expect(
    await adminCall('POST', `/branches/${branchId}/inventory-items/${item.id}/opening-stock`, {
      expectedVersion: 0,
      quantity,
      reason: 'Development opening stock',
    }),
    `open stock for ${item.name}`,
    [200],
  )
}

async function main() {
  requireDevelopment()
  assertLocalHost(apiBase, 'DEV_API_URL')
  assertDevDatabaseName()
  if (!bootstrapSecret) {
    fail('BOOTSTRAP_SECRET is required to provision the development organization (see .env.example).')
  }
  await waitForBackend()

  const bootstrap = await request('POST', '/bootstrap', {
    headers: { 'X-Bootstrap-Token': bootstrapSecret },
    data: {
      organization: {
        name: organizationName,
        code: organizationCode,
        timezone: 'America/Mexico_City',
        currency: 'MXN',
        locale: 'es-MX',
      },
      administrator: { email: adminEmail, password: adminPassword },
    },
  })
  if (bootstrap.status !== 201) {
    const text = await bootstrap.text().catch(() => '')
    fail(
      `Bootstrap failed with HTTP ${bootstrap.status}: ${text.slice(0, 300)}. ` +
        'The database is already provisioned; run `pnpm dev:reset` for a deterministic seed.',
    )
  }
  const { organizationId } = await bootstrap.json()
  const adminCall = admin(organizationId, await login(adminEmail, adminPassword))

  // Branches (>= 2 to exercise multi-branch behaviour).
  const branchA = await expect(await adminCall('POST', '/branches', { name: 'Sucursal Centro', code: 'dev-a' }), 'create branch A', [201])
  const branchB = await expect(await adminCall('POST', '/branches', { name: 'Sucursal Norte', code: 'dev-b' }), 'create branch B', [201])

  // Cash registers.
  await expect(await adminCall('POST', `/branches/${branchA.id}/cash-registers`, { name: 'Caja A1', code: 'dev-a1' }), 'create register A1', [201])
  await expect(await adminCall('POST', `/branches/${branchA.id}/cash-registers`, { name: 'Caja A2', code: 'dev-a2' }), 'create register A2', [201])
  await expect(await adminCall('POST', `/branches/${branchB.id}/cash-registers`, { name: 'Caja B1', code: 'dev-b1' }), 'create register B1', [201])

  // Roles: inventory admin (real inventory permissions) and cashier (POS subset).
  const catalog = await expect(await adminCall('GET', '/permissions', undefined), 'list permission catalog', [200])
  const allPermissionCodes = catalog.map(({ code }) => code)
  const missing = [...cashierPermissions, ...inventoryAdminPermissions].filter((code) => !allPermissionCodes.includes(code))
  if (missing.length) fail(`Required permissions missing from the catalog: ${missing.join(', ')}`)
  const inventoryAdminRole = await createRole(adminCall, 'dev-inventory-admin', 'Inventario (dev)', inventoryAdminPermissions)
  const cashierRole = await createRole(adminCall, 'dev-cashier', 'Cajero (dev)', cashierPermissions)

  // Users and memberships: manager runs the inventory admin UI; cashier runs the POS.
  const manager = await createUser(adminCall, managerEmail, staffPassword)
  const cashier = await createUser(adminCall, cashierEmail, staffPassword)
  // Manager: organization-wide inventory role (organization-scoped permissions)
  // plus a membership in both branches, so the branch list and stock screens are
  // discoverable. Cashier: POS role scoped to both branches.
  await expect(
    await adminCall('PUT', `/users/${manager.id}/roles`, { roleIds: [inventoryAdminRole.id] }),
    'grant manager organization role',
    [200],
  )
  await addMembership(adminCall, branchA.id, manager.id)
  await addMembership(adminCall, branchB.id, manager.id)
  await assignMembership(adminCall, branchA.id, cashier.id, cashierRole.id)
  await assignMembership(adminCall, branchB.id, cashier.id, cashierRole.id)

  // Catalog: categories and products.
  const drinks = await expect(await adminCall('POST', '/categories', { code: 'dev-coffee', name: 'Café' }), 'create category coffee', [201])
  await expect(await adminCall('POST', '/categories', { code: 'dev-tea', name: 'Té' }), 'create category tea', [201])
  const americano = await createProduct(adminCall, { sku: 'DEV-AMERICANO', name: 'Americano', unitPrice: '35.0000', categoryId: drinks.id })
  const latte = await createProduct(adminCall, { sku: 'DEV-LATTE', name: 'Latte', unitPrice: '45.0000', categoryId: drinks.id })
  await createProduct(adminCall, { sku: 'DEV-TEA', name: 'Té chai', unitPrice: '32.0000', categoryId: drinks.id })

  // Inventory items (exact decimal quantities; UNIT rejects fractions).
  const beans = await createItem(adminCall, { code: 'dev-beans', name: 'Café en grano', baseUnit: 'GRAM', fractionalUnitAllowed: true })
  const milk = await createItem(adminCall, { code: 'dev-milk', name: 'Leche entera', baseUnit: 'MILLILITER', fractionalUnitAllowed: true })
  const cups = await createItem(adminCall, { code: 'dev-cups', name: 'Vaso 12 oz', baseUnit: 'UNIT', fractionalUnitAllowed: false })

  // Opening stock on BOTH branches (multi-branch stock UI).
  for (const [item, quantity] of [[beans, '5000'], [milk, '5000'], [cups, '100']]) {
    await openStock(adminCall, branchA.id, item, quantity)
  }
  for (const [item, quantity] of [[beans, '2000'], [milk, '2000'], [cups, '50']]) {
    await openStock(adminCall, branchB.id, item, quantity)
  }

  // Recipes: an inventory-consuming sellable product (AMS + Latte).
  await expect(
    await adminCall('PUT', `/products/${americano.id}/inventory-recipe`, {
      expectedVersion: 0,
      status: 'ACTIVE',
      components: [{ inventoryItemId: beans.id, quantity: '18' }],
    }),
    'create americano recipe',
    [200],
  )
  await expect(
    await adminCall('PUT', `/products/${latte.id}/inventory-recipe`, {
      expectedVersion: 0,
      status: 'ACTIVE',
      components: [
        { inventoryItemId: beans.id, quantity: '18' },
        { inventoryItemId: milk.id, quantity: '200' },
        { inventoryItemId: cups.id, quantity: '1' },
      ],
    }),
    'create latte recipe',
    [200],
  )

  // Modifier group + options, including a signed inventory effect to test UI-B.
  const sizeGroup = await expect(
    await adminCall('POST', '/modifier-groups', { code: 'dev-size', name: 'Tamaño', minSelections: 1, maxSelections: 1 }),
    'create modifier group',
    [201],
  )
  await expect(
    await adminCall('POST', `/modifier-groups/${sizeGroup.id}/options`, { code: 'dev-size-regular', name: 'Regular', priceDelta: '0.0000', displayOrder: 0 }),
    'create regular option',
    [201],
  )
  const large = await expect(
    await adminCall('POST', `/modifier-groups/${sizeGroup.id}/options`, { code: 'dev-size-large', name: 'Grande', priceDelta: '10.0000', displayOrder: 1 }),
    'create large option',
    [201],
  )
  await expect(
    await adminCall('PUT', `/modifier-options/${large.id}/inventory-effects`, {
      expectedVersion: large.version ?? 0,
      effects: [{ inventoryItemId: milk.id, quantityDelta: '100' }],
    }),
    'create large option inventory effect',
    [200],
  )
  await expect(
    await adminCall('PUT', `/products/${latte.id}/modifier-groups`, {
      expectedVersion: latte.version,
      modifierGroupIds: [sizeGroup.id],
    }),
    'attach modifier group to latte',
    [200],
  )

  console.log('[dev] Development fixtures seeded.')
  console.log(`[dev]   organization : ${organizationCode} (${organizationId})`)
  console.log(`[dev]   branches     : ${branchA.name} (dev-a), ${branchB.name} (dev-b)`)
  console.log(`[dev]   admin        : ${adminEmail} / ${adminPassword}`)
  console.log(`[dev]   manager      : ${managerEmail} / ${staffPassword}`)
  console.log(`[dev]   cashier      : ${cashierEmail} / ${staffPassword}`)
  console.log(`[dev]   products     : ${americano.sku}, ${latte.sku} (tracked: recipe + modifier effect)`)
}

async function login(email, password) {
  const res = await request('POST', '/auth/login', { data: { organizationCode, email, password } })
  const body = await expect(res, 'admin login', [200])
  return body.accessToken
}

main().catch(handleTopLevel)
