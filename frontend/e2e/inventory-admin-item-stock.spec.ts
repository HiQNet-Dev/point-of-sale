import { expect, test } from '@playwright/test'
import { ADMIN_EMAIL, ADMIN_PASSWORD, createAdminApi, readGlobalFixtures, signInAt } from './support'

/**
 * Journey A — administración de Inventory.
 * Crea un artículo por UI y luego registra apertura, ajuste e historial de
 * existencias en una sucursal, verificando la persistencia exacta vía API.
 */
test('crea un artículo y registra apertura, ajuste e historial por sucursal', async ({ page }) => {
  const itemCode = 'e2e-admin-item'
  const itemName = 'E2E Admin Item'
  const adjustReason = 'Conteo físico E2E'
  const fixtures = await readGlobalFixtures()

  const admin = await createAdminApi()
  try {
    // ARRANGE: el artículo todavía no existe en la organización de prueba.
    expect(await admin.findInventoryItemByCode(itemCode)).toBeUndefined()

    // ACT: crear el artículo con la UI real.
    await signInAt(page, '/admin/inventory/items', ADMIN_EMAIL, ADMIN_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Artículos de inventario' })).toBeVisible()
    await page.getByRole('button', { name: 'Nuevo artículo' }).click()
    const createDialog = page.getByRole('dialog', { name: 'Nuevo artículo de inventario' })
    await createDialog.getByLabel('Código').fill(itemCode)
    await createDialog.getByLabel('Nombre').fill(itemName)
    await createDialog.getByRole('button', { name: 'Crear artículo' }).click()
    await expect(createDialog).toBeHidden()

    // ASSERT: aparece en la lista con código, nombre, unidad y estado correctos.
    const listRow = page.getByRole('row', { name: new RegExp(itemName) })
    await expect(listRow).toBeVisible()
    await expect(listRow).toContainText(itemCode)
    await expect(listRow).toContainText('Unidad')
    await expect(listRow).toContainText('Activo')

    // ASSERT: el artículo quedó persistido (no es solo estado local del render).
    const created = await admin.findInventoryItemByCode(itemCode)
    expect(created).toBeDefined()
    expect(created?.name).toBe(itemName)
    expect(created?.baseUnit).toBe('UNIT')
    expect(created?.fractionalUnitAllowed).toBe(false)
    expect(created?.status).toBe('ACTIVE')
    const itemId = created!.id

    // Navegación in-app (auth memory-only) hacia existencias por sucursal.
    await page.getByRole('link', { name: 'Existencias por sucursal' }).click()
    await expect(page.getByRole('heading', { name: 'Existencias por sucursal' })).toBeVisible()
    await page.getByRole('combobox', { name: 'Sucursal' }).click()
    await page.getByRole('option', { name: /E2E Branch A/ }).click()
    await page.getByLabel('Buscar').fill(itemCode)
    await page.getByRole('button', { name: 'Buscar' }).click()
    const stockRow = page.getByRole('row', { name: new RegExp(itemName) })
    await expect(stockRow).toBeVisible()
    await stockRow.getByRole('link', { name: 'Ver' }).click()

    // ACT: apertura de existencias por UI.
    await expect(page.getByRole('heading', { name: itemName })).toBeVisible()
    await page.getByRole('button', { name: 'Apertura de existencias' }).click()
    const openingDialog = page.getByRole('dialog', { name: 'Apertura de existencias' })
    await openingDialog.getByLabel('Cantidad de apertura').fill('100')
    await openingDialog.getByRole('button', { name: 'Registrar apertura' }).click()
    await expect(openingDialog).toBeHidden()
    // La apertura solo se permite antes de cualquier movimiento.
    await expect(page.getByRole('button', { name: 'Apertura de existencias' })).toBeDisabled()

    // ACT: ajuste firmado por UI.
    await page.getByRole('button', { name: 'Ajuste de existencias' }).click()
    const adjustDialog = page.getByRole('dialog', { name: 'Ajuste de existencias' })
    await adjustDialog.getByLabel('Ajuste (+/-)').fill('-20')
    await adjustDialog.getByRole('textbox', { name: 'Motivo' }).fill(adjustReason)
    await adjustDialog.getByRole('button', { name: 'Registrar ajuste' }).click()
    await expect(adjustDialog).toBeHidden()

    // ASSERT: la UI muestra el saldo resultante exacto (100 - 20 = 80).
    await expect(page.getByText('80 Unidad')).toBeVisible()

    // ASSERT: historial de movimientos en la UI (etiquetas humanas + motivo).
    const history = page.getByRole('table', { name: 'Historial de movimientos' })
    await expect(history.getByText('Apertura')).toBeVisible()
    await expect(history.getByText('Ajuste', { exact: true })).toBeVisible()
    await expect(history.getByText(adjustReason)).toBeVisible()

    // ASSERT: persistencia exacta (decimal strings NUMERIC(19,6), sin float).
    const stock = await admin.getStock(fixtures.branchAId, itemId)
    expect(stock.quantityOnHand).toBe('80.000000')

    const movements = await admin.listMovements(fixtures.branchAId, itemId)
    expect(movements.map((movement) => movement.type).sort()).toEqual(['ADJUSTMENT', 'OPENING'])
    const opening = movements.find((movement) => movement.type === 'OPENING')
    expect(opening?.quantityDelta).toBe('100.000000')
    const adjustment = movements.find((movement) => movement.type === 'ADJUSTMENT')
    expect(adjustment?.quantityDelta).toBe('-20.000000')
    expect(adjustment?.resultingQuantity).toBe('80.000000')
    expect(adjustment?.reason).toBe(adjustReason)
  } finally {
    await admin.dispose()
  }
})
