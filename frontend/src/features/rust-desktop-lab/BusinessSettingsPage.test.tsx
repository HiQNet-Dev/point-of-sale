// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { BusinessSettingsPage } from './BusinessSettingsPage'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'

vi.mock('../../shared/rust-local/desktop-client', async () => {
  const actual = await vi.importActual<typeof import('../../shared/rust-local/desktop-client')>(
    '../../shared/rust-local/desktop-client',
  )
  return {
    DesktopCommandError: actual.DesktopCommandError,
    desktopService: { organizationGet: vi.fn(), organizationUpdate: vi.fn() },
  }
})

afterEach(() => {
  cleanup()
  vi.resetAllMocks()
})

const organization = {
  id: 'org-1',
  code: 'cafe-central',
  name: 'Café Central',
  status: 'ACTIVE',
  timezone: 'America/Mexico_City',
  currency: 'MXN',
  locale: 'es-MX',
  negativeStockPolicy: 'STRICT',
  revision: '3',
}

const allPermissions = ['organization.read', 'organization.update']

function renderPage(permissions: string[] = allPermissions) {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <BusinessSettingsPage permissions={permissions} />
    </QueryClientProvider>,
  )
}

describe('Business settings (unit, service mocked)', () => {
  it('shows the organisation read from Rust and keeps immutable fields disabled', async () => {
    vi.mocked(desktopService.organizationGet).mockResolvedValue(organization)
    renderPage()

    const name = await screen.findByLabelText('Nombre comercial')
    expect((name as HTMLInputElement).value).toBe('Café Central')
    const code = screen.getByLabelText('Código de negocio') as HTMLInputElement
    expect(code.value).toBe('cafe-central')
    expect(code.disabled).toBe(true)
    expect((screen.getByLabelText('Moneda') as HTMLInputElement).disabled).toBe(true)
    expect((screen.getByLabelText('Estado') as HTMLInputElement).disabled).toBe(true)
    expect(screen.getByText(/Revisión 3/)).toBeDefined()
  })

  it('sends only the changed fields with the exact revision string', async () => {
    vi.mocked(desktopService.organizationGet).mockResolvedValue(organization)
    vi.mocked(desktopService.organizationUpdate).mockResolvedValue({
      ...organization,
      name: 'Café Central Norte',
      revision: '4',
    })
    renderPage()

    fireEvent.change(await screen.findByLabelText('Nombre comercial'), {
      target: { value: 'Café Central Norte' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar cambios' }))

    await waitFor(() =>
      expect(desktopService.organizationUpdate).toHaveBeenCalledWith({
        expectedRevision: '3',
        name: 'Café Central Norte',
        timezone: null,
        locale: null,
        negativeStockPolicy: null,
      }),
    )
    expect(await screen.findByText('Configuración guardada.')).toBeDefined()
    // La mutación invalida la consulta: se vuelve a leer del backend.
    await waitFor(() => expect(desktopService.organizationGet).toHaveBeenCalledTimes(2))
  })

  it('keeps the draft and explains a stale revision', async () => {
    vi.mocked(desktopService.organizationGet).mockResolvedValue(organization)
    vi.mocked(desktopService.organizationUpdate).mockRejectedValue(
      new DesktopCommandError('CONFLICT'),
    )
    renderPage()

    fireEvent.change(await screen.findByLabelText('Nombre comercial'), {
      target: { value: 'Otro nombre' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar cambios' }))

    expect(await screen.findByText(/Otra sesión cambió esta configuración/)).toBeDefined()
    expect((screen.getByLabelText('Nombre comercial') as HTMLInputElement).value).toBe('Otro nombre')
  })

  it('discards local edits back to the persisted values', async () => {
    vi.mocked(desktopService.organizationGet).mockResolvedValue(organization)
    renderPage()

    const name = await screen.findByLabelText('Nombre comercial')
    expect((screen.getByRole('button', { name: 'Descartar cambios' }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.change(name, { target: { value: 'Cambio' } })
    fireEvent.click(screen.getByRole('button', { name: 'Descartar cambios' }))
    expect((screen.getByLabelText('Nombre comercial') as HTMLInputElement).value).toBe('Café Central')
  })

  it('disables editing without organization.update', async () => {
    vi.mocked(desktopService.organizationGet).mockResolvedValue(organization)
    renderPage(['organization.read'])

    const name = await screen.findByLabelText('Nombre comercial')
    expect((name as HTMLInputElement).disabled).toBe(true)
    expect(screen.getByText(/no modificarla/)).toBeDefined()
    expect((screen.getByRole('button', { name: 'Guardar cambios' }) as HTMLButtonElement).disabled).toBe(true)
    expect(desktopService.organizationUpdate).not.toHaveBeenCalled()
  })

  it('reports an unauthorised read without retrying', async () => {
    vi.mocked(desktopService.organizationGet).mockRejectedValue(
      new DesktopCommandError('UNAUTHORIZED'),
    )
    renderPage()
    expect(await screen.findByText(/La sesión ya no es válida/)).toBeDefined()
    expect(desktopService.organizationGet).toHaveBeenCalledTimes(1)
  })
})
