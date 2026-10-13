import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import {
  CASHIER_EMAIL,
  CASHIER_PASSWORD,
  createAdminApi,
  readGlobalFixtures,
  signInAt,
} from './support'
import type { AdminApi } from './support'

/**
 * Journey B — checkout con stock insuficiente (release gate).
 * Verifica el rechazo controlado bajo STRICT con rollback atómico, y el camino
 * ALLOW_NEGATIVE (valioso) donde el checkout completa y el saldo queda negativo.
 */

async function openShift(page: Page, registerName: string): Promise<string> {
  const branchSelect = page.getByRole('combobox', { name: 'Sucursal' })
  await branchSelect.click()
  await page.getByRole('option', { name: /E2E Branch A/ }).click()
  const registerSelect = page.getByRole('combobox', { name: 'Caja' })
  await registerSelect.click()
  await page.getByRole('option', { name: new RegExp(registerName) }).click()

  await expect(page.getByRole('heading', { name: 'Sin turno activo' })).toBeVisible()
  await page.getByRole('button', { name: 'Abrir turno' }).click()
  await page.getByLabel('Fondo inicial').fill('100.0000')
  const shiftResponse = page.waitForResponse(
    (response) => response.request().method() === 'POST' && /\/cash-shifts$/.test(new URL(response.url()).pathname),
  )
  await page.getByRole('button', { name: 'Abrir turno' }).last().click()
  await expect(page.getByText('TURNO ACTIVO')).toBeVisible()
  const shift = (await (await shiftResponse).json()) as { id: string }
  return shift.id
}

async function createSale(page: Page): Promise<string> {
  const saleResponse = page.waitForResponse(
    (response) => response.request().method() === 'POST' && /\/sales$/.test(new URL(response.url()).pathname),
  )
  await page.getByRole('button', { name: 'Nueva venta' }).click()
  await expect(page.getByText('Tu venta está vacía.')).toBeVisible()
  const sale = (await (await saleResponse).json()) as { id: string }
  return sale.id
}

async function addProductToSale(page: Page, productName: string): Promise<void> {
  await page.getByRole('button', { name: new RegExp(productName) }).click()
  const dialog = page.getByRole('dialog', { name: /Agregar producto/ })
  await expect(dialog).toBeVisible()
  await dialog.getByRole('button', { name: 'Agregar a la venta' }).click()
  await expect(page.getByText(new RegExp(`1 × ${productName}`))).toBeVisible()
}

async function closeShift(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Cerrar turno' }).click()
  await page.getByLabel('Efectivo declarado').fill('100.0000')
  await page.getByRole('button', { name: 'Cerrar turno' }).last().click()
  await expect(page.getByRole('heading', { name: 'Sin turno activo' })).toBeVisible()
}

interface StrictSetup {
  admin: AdminApi
  branchId: string
  registerId: string
  productId: string
  productName: string
  enoughItemId: string
  shortItemId: string
}

/** Prepara producto rastreado con receta de 2 componentes y stock por sucursal. */
async function setupTrackedProduct(
  prefix: string,
  productName: string,
  registerName: string,
): Promise<StrictSetup> {
  const fixtures = await readGlobalFixtures()
  const admin = await createAdminApi()
  const branchId = fixtures.branchAId

  const category = await admin.createCategory(`${prefix}-cat`, `${prefix} Category`)
  const enough = await admin.createInventoryItem({ code: `${prefix}-beans`, name: `${productName} Beans`, baseUnit: 'GRAM' })
  const short = await admin.createInventoryItem({ code: `${prefix}-milk`, name: `${productName} Milk`, baseUnit: 'MILLILITER' })
  await admin.openStock(branchId, enough.id, { expectedVersion: 0, quantity: '100' })
  await admin.openStock(branchId, short.id, { expectedVersion: 0, quantity: '5' })

  const product = await admin.createProduct({
    sku: `${prefix.toUpperCase()}`,
    name: productName,
    unitPrice: '9.0000',
    categoryId: category.id,
  })
  await admin.setRecipe(product.id, {
    expectedVersion: 0,
    status: 'ACTIVE',
    components: [
      { inventoryItemId: enough.id, quantity: '10' },
      { inventoryItemId: short.id, quantity: '10' },
    ],
  })
  const register = await admin.createCashRegister(branchId, { name: registerName, code: `${prefix}-reg` })

  return {
    admin,
    branchId,
    registerId: register.id,
    productId: product.id,
    productName,
    enoughItemId: enough.id,
    shortItemId: short.id,
  }
}

test('rechaza el checkout bajo STRICT con stock insuficiente y rollback atómico', async ({ page }) => {
  const setup = await setupTrackedProduct('e2e-strict', 'E2E Strict Product', 'E2E Strict Register')
  const { admin, branchId, registerId } = setup
  try {
    // ARRANGE: política explícitamente STRICT.
    const organization = await admin.getOrganization()
    if (organization.inventoryNegativeStockPolicy !== 'STRICT') {
      await admin.setNegativeStockPolicy(organization.version, 'STRICT')
    }

    // ACT: sesión de caja real y checkout en el browser.
    await signInAt(page, '/pos', CASHIER_EMAIL, CASHIER_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()
    const shiftId = await openShift(page, 'E2E Strict Register')
    const saleId = await createSale(page)
    await addProductToSale(page, setup.productName)

    await page.getByRole('button', { name: 'Cobrar' }).click()
    await page.getByRole('combobox', { name: 'Método de pago' }).click()
    await page.getByRole('option', { name: 'Tarjeta' }).click()
    await page.getByRole('button', { name: /Usar restante/ }).click()
    const checkoutResponse = page.waitForResponse(
      (response) => response.request().method() === 'POST' && /\/checkout$/.test(new URL(response.url()).pathname),
    )
    await page.getByRole('button', { name: 'Completar venta' }).click()
    expect((await checkoutResponse).status()).toBe(409)

    // ASSERT UI: error controlado visible; la venta NO se muestra completada.
    const checkoutDialog = page.getByRole('dialog', { name: 'Cobrar venta' })
    await expect(checkoutDialog.getByRole('alert')).toContainText('No hay inventario suficiente')
    await expect(page.getByRole('status').filter({ hasText: 'Venta completada' })).toHaveCount(0)

    // ASSERT: la Sale sigue OPEN (sin falso COMPLETED).
    expect(await admin.getSaleStatus({ branchId, registerId, shiftId, saleId })).toBe('OPEN')

    // ASSERT: atomicidad — ningún saldo cambió y no hay SALE_CONSUMPTION persistido.
    const enough = await admin.getStock(branchId, setup.enoughItemId)
    const short = await admin.getStock(branchId, setup.shortItemId)
    expect(enough.quantityOnHand).toBe('100.000000')
    expect(short.quantityOnHand).toBe('5.000000')
    for (const itemId of [setup.enoughItemId, setup.shortItemId]) {
      const movements = await admin.listMovements(branchId, itemId)
      expect(movements.map((movement) => movement.type)).toEqual(['OPENING'])
      expect(movements.some((movement) => movement.saleId === saleId)).toBe(false)
    }

    // Cleanup: descartar la venta abierta y cerrar el turno.
    await page.getByRole('button', { name: 'Volver al carrito' }).click()
    await page.getByRole('button', { name: 'Cancelar venta' }).click()
    await page.getByRole('button', { name: 'Cancelar venta', exact: true }).last().click()
    await expect(page.getByText('No hay ventas abiertas.')).toBeVisible()
    await closeShift(page)
  } finally {
    await admin.dispose()
  }
})

test('permite el checkout bajo ALLOW_NEGATIVE y deja el saldo negativo', async ({ page }) => {
  const setup = await setupTrackedProduct('e2e-allow', 'E2E Allow Product', 'E2E Allow Register')
  const { admin, branchId, registerId } = setup
  try {
    // ARRANGE: política ALLOW_NEGATIVE para este escenario.
    const organization = await admin.getOrganization()
    await admin.setNegativeStockPolicy(organization.version, 'ALLOW_NEGATIVE')

    // ACT: mismo stock insuficiente, pero el checkout debe completar.
    await signInAt(page, '/pos', CASHIER_EMAIL, CASHIER_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()
    const shiftId = await openShift(page, 'E2E Allow Register')
    const saleId = await createSale(page)
    await addProductToSale(page, setup.productName)

    await page.getByRole('button', { name: 'Cobrar' }).click()
    await page.getByRole('combobox', { name: 'Método de pago' }).click()
    await page.getByRole('option', { name: 'Tarjeta' }).click()
    await page.getByRole('button', { name: /Usar restante/ }).click()
    await page.getByRole('button', { name: 'Completar venta' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'Venta completada' })).toBeVisible()

    // ASSERT: la venta completó y el inventario quedó negativo.
    expect(await admin.getSaleStatus({ branchId, registerId, shiftId, saleId })).toBe('COMPLETED')
    const enough = await admin.getStock(branchId, setup.enoughItemId)
    const short = await admin.getStock(branchId, setup.shortItemId)
    expect(enough.quantityOnHand).toBe('90.000000')
    expect(short.quantityOnHand).toBe('-5.000000')

    // ASSERT: se persistió un SALE_CONSUMPTION por componente, ligado a la venta.
    const shortMovements = await admin.listMovements(branchId, setup.shortItemId)
    const consumption = shortMovements.find((movement) => movement.type === 'SALE_CONSUMPTION')
    expect(consumption?.quantityDelta).toBe('-10.000000')
    expect(consumption?.resultingQuantity).toBe('-5.000000')
    expect(consumption?.saleId).toBe(saleId)

    await closeShift(page)
  } finally {
    // Restaurar STRICT para no alterar el resto de la suite.
    const organization = await admin.getOrganization()
    if (organization.inventoryNegativeStockPolicy !== 'STRICT') {
      await admin.setNegativeStockPolicy(organization.version, 'STRICT')
    }
    await admin.dispose()
  }
})
