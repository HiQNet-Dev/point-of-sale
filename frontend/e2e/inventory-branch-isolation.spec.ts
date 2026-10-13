import { expect, test } from '@playwright/test'
import { ADMIN_EMAIL, ADMIN_PASSWORD, createAdminApi, readGlobalFixtures, signInAt } from './support'

/**
 * Journey C — aislamiento multisucursal (release gate).
 * Demuestra que el saldo de una Branch no se mezcla con otra: la UI cambia de
 * sucursal sin valores stale y una mutación en A no afecta a B.
 */
test('mantiene aisladas las existencias entre sucursales', async ({ page }) => {
  const itemCode = 'e2e-iso-item'
  const itemName = 'E2E Isolation Item'
  const fixtures = await readGlobalFixtures()

  const admin = await createAdminApi()
  try {
    // ARRANGE: mismo artículo con saldos distintos por sucursal (A=100, B=25).
    const item = await admin.createInventoryItem({ code: itemCode, name: itemName, baseUnit: 'UNIT' })
    await admin.openStock(fixtures.branchAId, item.id, { expectedVersion: 0, quantity: '100' })
    await admin.openStock(fixtures.branchBId, item.id, { expectedVersion: 0, quantity: '25' })
    expect((await admin.getStock(fixtures.branchAId, item.id)).quantityOnHand).toBe('100.000000')
    expect((await admin.getStock(fixtures.branchBId, item.id)).quantityOnHand).toBe('25.000000')

    // ACT: consultar la sucursal A por UI.
    await signInAt(page, '/admin/inventory/stock', ADMIN_EMAIL, ADMIN_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Existencias por sucursal' })).toBeVisible()
    await page.getByRole('combobox', { name: 'Sucursal' }).click()
    await page.getByRole('option', { name: /E2E Branch A/ }).click()
    await page.getByLabel('Buscar').fill(itemCode)
    await page.getByRole('button', { name: 'Buscar' }).click()
    const rowA = page.getByRole('row', { name: new RegExp(itemName) })
    await expect(rowA).toContainText('100')

    // ACT: cambiar a la sucursal B.
    await page.getByRole('combobox', { name: 'Sucursal' }).click()
    await page.getByRole('option', { name: /E2E Branch B/ }).click()

    // ASSERT: muestra el saldo de B y NO el valor stale de A.
    const rowB = page.getByRole('row', { name: new RegExp(itemName) })
    await expect(rowB).toContainText('25')
    await expect(rowB).not.toContainText('100')

    // ACT: volver a A y mutar (-10) en el detalle.
    await page.getByRole('combobox', { name: 'Sucursal' }).click()
    await page.getByRole('option', { name: /E2E Branch A/ }).click()
    const rowAAgain = page.getByRole('row', { name: new RegExp(itemName) })
    await expect(rowAAgain).toContainText('100')
    await rowAAgain.getByRole('link', { name: 'Ver' }).click()
    await expect(page.getByRole('heading', { name: itemName })).toBeVisible()
    await page.getByRole('button', { name: 'Ajuste de existencias' }).click()
    const adjustDialog = page.getByRole('dialog', { name: 'Ajuste de existencias' })
    await adjustDialog.getByLabel('Ajuste (+/-)').fill('-10')
    await adjustDialog.getByRole('textbox', { name: 'Motivo' }).fill('Ajuste aislado E2E')
    await adjustDialog.getByRole('button', { name: 'Registrar ajuste' }).click()
    await expect(adjustDialog).toBeHidden()
    await expect(page.getByText('90 Unidad')).toBeVisible()

    // ASSERT: estado persistido — A cambió, B permaneció intacta.
    expect((await admin.getStock(fixtures.branchAId, item.id)).quantityOnHand).toBe('90.000000')
    expect((await admin.getStock(fixtures.branchBId, item.id)).quantityOnHand).toBe('25.000000')

    // ASSERT: el ledger tampoco se cruzó entre sucursales.
    expect((await admin.listMovements(fixtures.branchAId, item.id)).map((movement) => movement.type).sort())
      .toEqual(['ADJUSTMENT', 'OPENING'])
    expect((await admin.listMovements(fixtures.branchBId, item.id)).map((movement) => movement.type))
      .toEqual(['OPENING'])
  } finally {
    await admin.dispose()
  }
})
