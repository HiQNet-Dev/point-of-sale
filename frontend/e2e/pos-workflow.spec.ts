import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'

const fixtureFile = join(tmpdir(), 'hiqnet-pos-e2e-fixture.json')
const cashierPassword = process.env.E2E_CASHIER_PASSWORD ?? ''

type Fixtures = {
  organizationCode: string
  organizationId: string
  branchBId: string
  cashierEmail: string
  noCatalogEmail: string
  noMutationEmail: string
  simpleProductName: string
  zeroProductName: string
  requiredProductName: string
  requiredOptionName: string
  multipleProductName: string
  activeOptionOneName: string
  activeOptionTwoName: string
  inactiveOptionName: string
  inactiveCategoryProductName: string
}

async function signIn(page: import('@playwright/test').Page, email = process.env.E2E_CASHIER_EMAIL ?? 'cashier@pos-e2e.invalid') {
  await page.getByLabel('Organization code').fill(process.env.E2E_ORGANIZATION_CODE ?? 'pos-e2e')
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password').fill(cashierPassword)
  await page.getByRole('button', { name: 'Sign in' }).click()
}

async function newSale(page: import('@playwright/test').Page, requests: string[]) {
  const before = requests.length
  await page.getByRole('button', { name: 'Nueva venta' }).click()
  await expect.poll(() => requests.length - before).toBe(1)
  await expect(page.getByText('Tu venta está vacía.')).toBeVisible()
}

async function addProduct(page: import('@playwright/test').Page, productName: string) {
  await page.getByRole('button', { name: new RegExp(productName) }).click()
  await expect(page.getByRole('dialog', { name: /Agregar producto/ })).toBeVisible()
}

async function completeCheckout(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: 'Completar venta' }).click()
  await expect(page.getByRole('status').filter({ hasText: 'Venta completada' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Nueva venta' })).toBeVisible()
}

test('cashier can recover POS, configure products and complete sales without payment.read', async ({ page, browser, request }) => {
  const fixtures = JSON.parse(await readFile(fixtureFile, 'utf8')) as Fixtures
  const apiBase = process.env.E2E_API_URL ?? 'http://127.0.0.1:8080/api/v1'
  const authResponse = await request.post(`${apiBase}/auth/login`, {
    data: {
      organizationCode: fixtures.organizationCode,
      email: fixtures.cashierEmail,
      password: cashierPassword,
    },
  })
  expect(authResponse.status()).toBe(200)
  const accessToken = (await authResponse.json() as { accessToken: string }).accessToken
  const bearer = { Authorization: `Bearer ${accessToken}` }
  const crossBranchCatalog = await request.get(
    `${apiBase}/organizations/${fixtures.organizationId}/branches/${fixtures.branchBId}/pos/catalog/products?page=0&size=20`,
    { headers: bearer },
  )
  expect(crossBranchCatalog.status()).toBe(403)
  const forbiddenProductWrite = await request.post(`${apiBase}/organizations/${fixtures.organizationId}/products`, {
    headers: bearer,
    data: { sku: 'POS-ILLEGAL', name: 'Synthetic denied write', unitPrice: '1.0000' },
  })
  expect(forbiddenProductWrite.status()).toBe(403)
  const forbiddenUserWrite = await request.post(`${apiBase}/organizations/${fixtures.organizationId}/users`, {
    headers: bearer,
    data: { email: 'forbidden-write@pos-e2e.invalid', password: cashierPassword },
  })
  expect(forbiddenUserWrite.status()).toBe(403)

  const saleCreates: string[] = []
  const checkoutBodies: Array<{ expectedSaleVersion: number; payments: Array<{ method: string; amount: string; tenderedAmount?: string }> }> = []
  let paymentReads = 0
  page.on('request', (browserRequest) => {
    if (browserRequest.method() === 'POST' && /\/cash-shifts\/[^/]+\/sales$/.test(browserRequest.url())) {
      saleCreates.push(browserRequest.url())
    }
    if (browserRequest.method() === 'POST' && /\/sales\/[^/]+\/checkout$/.test(browserRequest.url())) {
      checkoutBodies.push(browserRequest.postDataJSON())
    }
    if (/\/sales\/[^/]+\/payments(?:\?|$)/.test(browserRequest.url())) paymentReads += 1
  })

  await page.goto('/pos')
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible()
  await page.getByLabel('Organization code').fill(fixtures.organizationCode)
  await page.getByLabel('Email').fill(fixtures.cashierEmail)
  await page.getByLabel('Password').fill('known-wrong-e2e-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.getByRole('alert')).toContainText('The organization code, email, or password is incorrect.')
  await signIn(page, fixtures.cashierEmail)
  await expect(page.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()

  // Credentials are memory-only across reloads.
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible()
  await signIn(page, fixtures.cashierEmail)
  await expect(page.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()

  const branchSelect = page.getByRole('combobox', { name: 'Sucursal' })
  await expect(branchSelect).toBeVisible()
  await branchSelect.click()
  await page.getByRole('option', { name: /E2E Branch A/ }).click()
  const registerSelect = page.getByRole('combobox', { name: 'Caja' })
  await registerSelect.click()
  await page.getByRole('option', { name: /E2E Register A1/ }).click()

  await expect(page.getByRole('heading', { name: 'Sin turno activo' })).toBeVisible()
  await page.getByRole('button', { name: 'Abrir turno' }).click()
  await page.getByLabel('Fondo inicial').fill('100.0000')
  await page.getByRole('button', { name: 'Abrir turno' }).last().click()
  await expect(page.getByText('TURNO ACTIVO')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Ventas abiertas' })).toBeVisible()
  expect(saleCreates).toHaveLength(0)
  await expect(page.getByText('No hay ventas abiertas.')).toBeVisible()

  await newSale(page, saleCreates)
  await addProduct(page, fixtures.simpleProductName)
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await expect(page.getByText(new RegExp(`1 × ${fixtures.simpleProductName}`))).toBeVisible()

  const noCatalogPage = await browser.newPage()
  await noCatalogPage.goto('/pos')
  await signIn(noCatalogPage, fixtures.noCatalogEmail)
  await expect(noCatalogPage.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()
  const noCatalogBranch = noCatalogPage.getByRole('combobox', { name: 'Sucursal' })
  await noCatalogBranch.click()
  await noCatalogPage.getByRole('option', { name: /E2E Branch A/ }).click()
  const noCatalogRegister = noCatalogPage.getByRole('combobox', { name: 'Caja' })
  await noCatalogRegister.click()
  await noCatalogPage.getByRole('option', { name: /E2E Register A1/ }).click()
  await expect(noCatalogPage.getByText(new RegExp(`1 × ${fixtures.simpleProductName}`))).toBeVisible()
  await expect(noCatalogPage.getByText('No tienes permiso para consultar el catálogo de esta sucursal.').first()).toBeVisible()
  await noCatalogPage.close()

  const noMutationPage = await browser.newPage()
  await noMutationPage.goto('/pos')
  await signIn(noMutationPage, fixtures.noMutationEmail)
  await expect(noMutationPage.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()
  const noMutationBranch = noMutationPage.getByRole('combobox', { name: 'Sucursal' })
  await noMutationBranch.click()
  await noMutationPage.getByRole('option', { name: /E2E Branch A/ }).click()
  const noMutationRegister = noMutationPage.getByRole('combobox', { name: 'Caja' })
  await noMutationRegister.click()
  await noMutationPage.getByRole('option', { name: /E2E Register A1/ }).click()
  await expect(noMutationPage.getByText(new RegExp(`1 × ${fixtures.simpleProductName}`))).toBeVisible()
  await noMutationPage.getByRole('button', { name: new RegExp(fixtures.simpleProductName) }).click()
  const rejectedAdd = noMutationPage.waitForResponse((response) =>
    response.request().method() === 'POST' && /\/sales\/[^/]+\/items$/.test(response.url()),
  )
  await noMutationPage.getByRole('button', { name: 'Agregar a la venta' }).click()
  expect((await rejectedAdd).status()).toBe(403)
  await expect(noMutationPage.getByRole('status').filter({ hasText: 'No tienes permiso para realizar esta operación.' })).toBeVisible()
  await noMutationPage.close()

  // Recover the OPEN Shift and persisted Sale/cart after memory-only auth is lost.
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible()
  await signIn(page, fixtures.cashierEmail)
  await expect(page.getByRole('heading', { name: 'Punto de venta' })).toBeVisible()
  const recoveredBranch = page.getByRole('combobox', { name: 'Sucursal' })
  await recoveredBranch.click()
  await page.getByRole('option', { name: /E2E Branch A/ }).click()
  const recoveredRegister = page.getByRole('combobox', { name: 'Caja' })
  await recoveredRegister.click()
  await page.getByRole('option', { name: /E2E Register A1/ }).click()
  await expect(page.getByText('TURNO ACTIVO')).toBeVisible()
  await expect(page.getByText(new RegExp(`1 × ${fixtures.simpleProductName}`))).toBeVisible()

  await page.getByRole('button', { name: 'Cobrar' }).click()
  await page.getByRole('combobox', { name: 'Método de pago' }).click()
  await page.getByRole('option', { name: 'Efectivo' }).click()
  await page.getByRole('button', { name: /Usar restante/ }).click()
  await page.getByLabel('Efectivo recibido').fill('20.0000')
  await expect(page.getByRole('status').filter({ hasText: 'Cambio estimado' })).toBeVisible()
  await completeCheckout(page)
  expect(checkoutBodies.at(-1)?.payments).toEqual([
    { method: 'CASH', amount: '10.1000', tenderedAmount: '20.0000' },
  ])

  await newSale(page, saleCreates)
  await addProduct(page, fixtures.requiredProductName)
  const addButton = page.getByRole('button', { name: 'Agregar a la venta' })
  await expect(addButton).toBeDisabled()
  await page.getByLabel(fixtures.requiredOptionName).check()
  await expect(addButton).toBeEnabled()
  await addButton.click()
  await expect(page.getByText(new RegExp(`1 × ${fixtures.requiredProductName}`))).toBeVisible()
  await page.getByRole('button', { name: 'Editar ' + fixtures.requiredProductName }).click()
  await page.getByRole('button', { name: 'Guardar cambios' }).click()
  await expect(page.getByText('Required choice: ' + fixtures.requiredOptionName)).toBeVisible()
  await addProduct(page, fixtures.multipleProductName)
  await page.getByLabel(fixtures.activeOptionOneName).check()
  await page.getByLabel(fixtures.activeOptionTwoName).check()
  await expect(page.getByLabel(fixtures.inactiveOptionName)).toBeDisabled()
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await expect(page.getByText('Multiple choice: Positive option')).toBeVisible()
  await expect(page.getByText('Multiple choice: Negative option')).toBeVisible()
  await page.getByRole('button', { name: 'Quitar ' + fixtures.requiredProductName }).click()
  await page.getByRole('button', { name: 'Quitar', exact: true }).click()
  await expect(page.getByText('Elementos retirados (1)')).toBeVisible()
  await expect(page.getByText('Agrega al menos un producto antes de cobrar.')).toBeVisible()

  // A fresh explicit sale is required after an item was removed.
  await page.getByRole('button', { name: 'Cancelar venta' }).click()
  await page.getByRole('button', { name: 'Cancelar venta', exact: true }).last().click()
  await expect(page.getByText('No hay ventas abiertas.')).toBeVisible()
  await newSale(page, saleCreates)
  await addProduct(page, fixtures.inactiveCategoryProductName)
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await expect(page.getByText(new RegExp(`1 × ${fixtures.inactiveCategoryProductName}`))).toBeVisible()

  // CARD is only a recorded tender method; successful checkout must not query history.
  await page.getByRole('button', { name: 'Cobrar' }).click()
  await page.getByRole('combobox', { name: 'Método de pago' }).click()
  await page.getByRole('option', { name: 'Tarjeta' }).click()
  await page.getByRole('button', { name: /Usar restante/ }).click()
  await completeCheckout(page)
  expect(checkoutBodies.at(-1)?.payments).toHaveLength(1)
  expect(checkoutBodies.at(-1)?.payments[0]).toMatchObject({ method: 'CARD', amount: '0.2000' })
  expect(checkoutBodies.at(-1)?.payments[0]).not.toHaveProperty('tenderedAmount')
  expect(paymentReads).toBe(0)
  expect(saleCreates).toHaveLength(3)

  await newSale(page, saleCreates)
  await addProduct(page, fixtures.simpleProductName)
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await page.getByRole('button', { name: 'Cobrar' }).click()
  const splitMethods = page.getByRole('combobox', { name: 'Método de pago' })
  await splitMethods.first().click()
  await page.getByRole('option', { name: 'Tarjeta' }).click()
  await page.getByLabel('Monto aplicado').first().fill('4.1000')
  await page.getByRole('button', { name: 'Agregar método de pago' }).click()
  await splitMethods.nth(1).click()
  await page.getByRole('option', { name: 'Efectivo' }).click()
  await page.getByRole('button', { name: /Usar restante/ }).last().click()
  await page.getByLabel('Efectivo recibido').fill('10.0000')
  await expect(page.getByRole('status').filter({ hasText: 'Cambio estimado' })).toBeVisible()
  await completeCheckout(page)
  expect(checkoutBodies.at(-1)?.payments).toEqual([
    { method: 'CARD', amount: '4.1000' },
    { method: 'CASH', amount: '6.0000', tenderedAmount: '10.0000' },
  ])

  await newSale(page, saleCreates)
  await addProduct(page, fixtures.simpleProductName)
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await page.getByRole('button', { name: 'Cobrar' }).click()
  const cardMethods = page.getByRole('combobox', { name: 'Método de pago' })
  await cardMethods.first().click()
  await page.getByRole('option', { name: 'Tarjeta' }).click()
  await page.getByLabel('Monto aplicado').first().fill('3.1000')
  await page.getByRole('button', { name: 'Agregar método de pago' }).click()
  await cardMethods.nth(1).click()
  await page.getByRole('option', { name: 'Tarjeta' }).click()
  await page.getByRole('button', { name: /Usar restante/ }).last().click()
  await completeCheckout(page)
  expect(checkoutBodies.at(-1)?.payments).toEqual([
    { method: 'CARD', amount: '3.1000' },
    { method: 'CARD', amount: '7.0000' },
  ])
  expect(saleCreates).toHaveLength(5)

  // Zero total is finalized with an empty payment array; no implicit next sale.
  await newSale(page, saleCreates)
  await addProduct(page, fixtures.zeroProductName)
  await page.getByRole('button', { name: 'Agregar a la venta' }).click()
  await page.getByRole('button', { name: 'Cobrar' }).click()
  await expect(page.getByText('Esta venta no requiere pagos.')).toBeVisible()
  await completeCheckout(page)
  expect(checkoutBodies.at(-1)?.payments).toEqual([])
  expect(saleCreates).toHaveLength(6)
  expect(paymentReads).toBe(0)

  // Responsive smoke against the running POS, not a mocked DOM.
  for (const viewport of [{ width: 360, height: 800 }, { width: 768, height: 1024 }, { width: 1920, height: 1080 }]) {
    await page.setViewportSize(viewport)
    const dimensions = await page.evaluate(() => ({
      viewport: document.documentElement.clientWidth,
      content: document.documentElement.scrollWidth,
    }))
    expect(dimensions.content).toBeLessThanOrEqual(dimensions.viewport)
  }

  // Close the operational shift only after all sales have reached terminal state.
  await page.getByRole('button', { name: 'Cerrar turno' }).click()
  await page.getByLabel('Efectivo declarado').fill('100.0000')
  await page.getByRole('button', { name: 'Cerrar turno' }).last().click()
  await expect(page.getByRole('heading', { name: 'Sin turno activo' })).toBeVisible()

})
