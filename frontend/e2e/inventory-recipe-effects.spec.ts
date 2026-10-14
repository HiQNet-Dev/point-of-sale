import { expect, test } from '@playwright/test'
import { ADMIN_EMAIL, ADMIN_PASSWORD, createAdminApi, signInAt } from './support'

/**
 * Journey A — configuración de recetas y efectos de modificadores por UI,
 * verificando la persistencia exacta vía API. Los prerrequisitos (categoría,
 * artículo, producto, grupo y opción) se crean por API porque su CRUD ya está
 * cubierto por otros tests; el comportamiento bajo prueba ocurre en la UI.
 */
test('configura y persiste la receta de un producto', async ({ page }) => {
  const itemCode = 'e2e-recipe-item'
  const itemName = 'E2E Recipe Item'
  const productSku = 'E2E-RECIPE'
  const productName = 'E2E Recipe Product'

  const admin = await createAdminApi()
  try {
    // ARRANGE desde la API (prerrequisitos).
    const category = await admin.createCategory('e2e-recipe-cat', 'E2E Recipe Category')
    await admin.createInventoryItem({ code: itemCode, name: itemName, baseUnit: 'GRAM' })
    const product = await admin.createProduct({
      sku: productSku,
      name: productName,
      unitPrice: '5.0000',
      categoryId: category.id,
    })

    // ACT: configurar la receta por UI.
    await signInAt(page, '/admin/inventory/recipes', ADMIN_EMAIL, ADMIN_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Recetas de inventario' })).toBeVisible()
    await page.getByLabel('Buscar producto').fill(productSku)
    await page.getByRole('button', { name: 'Buscar' }).click()
    const productRow = page.getByRole('row', { name: new RegExp(productName) })
    await expect(productRow).toBeVisible()
    await productRow.getByRole('button', { name: 'Configurar receta' }).click()

    await expect(page.getByRole('heading', { name: productName })).toBeVisible()
    await page.getByRole('button', { name: 'Añadir componente' }).click()
    await page.getByRole('combobox', { name: 'Artículo' }).click()
    await page.getByRole('combobox', { name: 'Artículo' }).fill(itemName)
    await page.getByRole('option', { name: new RegExp(itemCode) }).click()
    await page.getByRole('textbox', { name: 'Cantidad' }).fill('250')
    await page.getByRole('button', { name: 'Guardar receta' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'Receta guardada.' })).toBeVisible()

    // ASSERT: persistencia exacta vía API (cantidad como decimal string).
    const recipe = await admin.getRecipe(product.id)
    expect(recipe.status).toBe('ACTIVE')
    expect(recipe.components).toHaveLength(1)
    expect(recipe.components[0].code).toBe(itemCode)
    expect(recipe.components[0].quantity).toBe('250.000000')

    // ASSERT: al salir y volver, la UI rehidrata la receta desde el servidor.
    await page.getByRole('link', { name: 'Artículos', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Artículos de inventario' })).toBeVisible()
    await page.getByRole('link', { name: 'Recetas', exact: true }).click()
    await expect(page.getByRole('heading', { name: 'Recetas de inventario' })).toBeVisible()
    await page.getByLabel('Buscar producto').fill(productSku)
    await page.getByRole('button', { name: 'Buscar' }).click()
    await page.getByRole('row', { name: new RegExp(productName) }).getByRole('button', { name: 'Configurar receta' }).click()
    await expect(page.getByRole('combobox', { name: 'Artículo' })).toHaveValue(`${itemName} · ${itemCode}`)
    await expect(page.getByRole('textbox', { name: 'Cantidad' })).toHaveValue('250')
  } finally {
    await admin.dispose()
  }
})

test('configura y persiste un efecto de modificador', async ({ page }) => {
  const itemCode = 'e2e-effect-item'
  const itemName = 'E2E Effect Item'
  const groupCode = 'e2e-effect-group'
  const groupName = 'E2E Effect Group'
  const optionCode = 'e2e-effect-option'
  const optionName = 'E2E Effect Option'

  const admin = await createAdminApi()
  try {
    // ARRANGE desde la API (prerrequisitos).
    await admin.createInventoryItem({ code: itemCode, name: itemName, baseUnit: 'MILLILITER' })
    const group = await admin.createModifierGroup({ code: groupCode, name: groupName, minSelections: 0, maxSelections: 1 })
    const option = await admin.createModifierOption(group.id, {
      code: optionCode,
      name: optionName,
      priceDelta: '0.0000',
      displayOrder: 0,
    })

    // ACT: configurar el efecto por UI.
    await signInAt(page, '/admin/inventory/modifier-effects', ADMIN_EMAIL, ADMIN_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Efectos de modificadores' })).toBeVisible()
    await page.getByRole('combobox', { name: 'Grupo de modificadores' }).click()
    await page.getByRole('combobox', { name: 'Grupo de modificadores' }).fill(groupName)
    await page.getByRole('option', { name: new RegExp(groupCode) }).click()
    await page.getByRole('combobox', { name: 'Opción de modificador' }).click()
    await page.getByRole('combobox', { name: 'Opción de modificador' }).fill(optionName)
    await page.getByRole('option', { name: new RegExp(optionCode) }).click()
    await expect(page.getByRole('heading', { name: optionName })).toBeVisible()

    await page.getByRole('button', { name: 'Añadir efecto' }).click()
    await page.getByRole('combobox', { name: 'Artículo' }).click()
    await page.getByRole('combobox', { name: 'Artículo' }).fill(itemName)
    await page.getByRole('option', { name: new RegExp(itemCode) }).click()
    await page.getByRole('textbox', { name: 'Cambio de consumo' }).fill('-50')
    await page.getByRole('button', { name: 'Guardar efectos' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'Efectos guardados.' })).toBeVisible()

    // ASSERT: persistencia exacta vía API.
    const effects = await admin.getModifierEffects(option.id)
    expect(effects.effects).toHaveLength(1)
    expect(effects.effects[0].code).toBe(itemCode)
    expect(effects.effects[0].quantityDelta).toBe('-50.000000')
  } finally {
    await admin.dispose()
  }
})
