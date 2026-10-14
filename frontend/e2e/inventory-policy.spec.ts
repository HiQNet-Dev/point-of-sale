import { expect, test } from '@playwright/test'
import { ADMIN_EMAIL, ADMIN_PASSWORD, createAdminApi, signInAt } from './support'

/**
 * Journey A — política de stock negativo a nivel de organización.
 * Lee la política, la cambia por UI y verifica que persiste, dejándola en STRICT
 * para no alterar el resto de la suite.
 */
test('lee y actualiza la política de stock negativo de la organización', async ({ page }) => {
  const admin = await createAdminApi()
  try {
    // ARRANGE: partir explícitamente de STRICT (determinista, sin asumir el default).
    const current = await admin.getOrganization()
    if (current.inventoryNegativeStockPolicy !== 'STRICT') {
      await admin.setNegativeStockPolicy(current.version, 'STRICT')
    }

    // ACT: abrir la pantalla de configuración por UI.
    await signInAt(page, '/admin/inventory/settings', ADMIN_EMAIL, ADMIN_PASSWORD)
    await expect(page.getByRole('heading', { name: 'Configuración de inventario' })).toBeVisible()
    const strictRadio = page.getByRole('radio', { name: /^No permitir stock negativo/ })
    const allowRadio = page.getByRole('radio', { name: /^Permitir stock negativo/ })
    await expect(strictRadio).toBeChecked()
    await expect(allowRadio).not.toBeChecked()
    // El enum técnico se muestra como chip secundario.
    await expect(page.getByText('STRICT', { exact: true })).toBeVisible()

    // ACT: STRICT → ALLOW_NEGATIVE.
    await allowRadio.check()
    await page.getByRole('button', { name: 'Guardar configuración' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'Configuración guardada.' })).toBeVisible()

    // ASSERT: persistió en el servidor.
    const allowed = await admin.getOrganization()
    expect(allowed.inventoryNegativeStockPolicy).toBe('ALLOW_NEGATIVE')
    await expect(allowRadio).toBeChecked()

    // ACT: ALLOW_NEGATIVE → STRICT (deja la suite en el estado esperado).
    await strictRadio.check()
    await page.getByRole('button', { name: 'Guardar configuración' }).click()
    await expect(page.getByRole('status').filter({ hasText: 'Configuración guardada.' })).toBeVisible()

    // ASSERT: persistió de vuelta en STRICT.
    const restored = await admin.getOrganization()
    expect(restored.inventoryNegativeStockPolicy).toBe('STRICT')
    await expect(strictRadio).toBeChecked()
  } finally {
    await admin.dispose()
  }
})
