// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { BranchesPage } from './BranchesPage'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'

vi.mock('../../shared/rust-local/desktop-client', async () => {
  const actual = await vi.importActual<typeof import('../../shared/rust-local/desktop-client')>(
    '../../shared/rust-local/desktop-client',
  )
  return {
    DesktopCommandError: actual.DesktopCommandError,
    desktopService: { branchList: vi.fn(), branchCreate: vi.fn(), branchUpdate: vi.fn() },
  }
})

afterEach(() => {
  cleanup()
  vi.resetAllMocks()
})

const branch = {
  id: 'branch-1',
  code: 'centro',
  name: 'Sucursal Centro',
  status: 'ACTIVE',
  timezoneOverride: null,
  effectiveTimezone: 'America/Mexico_City',
  inheritsTimezone: true,
  revision: '2',
}

const allPermissions = ['branch.read', 'branch.create', 'branch.update']

function renderPage(permissions: string[] = allPermissions) {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <BranchesPage permissions={permissions} />
    </QueryClientProvider>,
  )
}

/** MUI renderiza el select como combobox con listbox en portal. */
async function chooseTimezoneMode(label: string) {
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: 'Zona horaria' }))
  fireEvent.click(await screen.findByRole('option', { name: label }))
}

describe('Branch administration (unit, service mocked)', () => {
  it('offers the initial branch when the installation has none', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([])
    vi.mocked(desktopService.branchCreate).mockResolvedValue({ ...branch, revision: '0' })
    renderPage()

    expect(await screen.findByText('Sucursal inicial')).toBeDefined()
    fireEvent.change(screen.getByLabelText('Código de la sucursal'), { target: { value: ' centro ' } })
    fireEvent.change(screen.getByLabelText('Nombre de la sucursal'), { target: { value: 'Sucursal Centro' } })
    fireEvent.click(screen.getByRole('button', { name: 'Crear sucursal' }))

    await waitFor(() =>
      expect(desktopService.branchCreate).toHaveBeenCalledWith({
        code: 'centro',
        name: 'Sucursal Centro',
        timezone: null,
      }),
    )
    await waitFor(() => expect(desktopService.branchList).toHaveBeenCalledTimes(2))
  })

  it('shows the effective timezone and marks it as inherited', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([branch])
    renderPage()
    expect(await screen.findByText(/America\/Mexico_City · heredada del negocio/)).toBeDefined()
    expect(screen.getByText(/centro \(no editable\)/)).toBeDefined()
    // La capacidad de Free Local se explica y no se ofrece crear otra.
    expect(screen.getByText(/Free Local admite una única sucursal/)).toBeDefined()
    expect(screen.queryByRole('button', { name: 'Crear sucursal' })).toBeNull()
  })

  it('sends an explicit SET operation with the branch revision', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([branch])
    vi.mocked(desktopService.branchUpdate).mockResolvedValue({
      ...branch,
      timezoneOverride: 'America/Cancun',
      effectiveTimezone: 'America/Cancun',
      inheritsTimezone: false,
      revision: '3',
    })
    renderPage()

    await chooseTimezoneMode('Definir una zona propia')
    fireEvent.change(screen.getByLabelText('Zona horaria propia'), {
      target: { value: 'America/Cancun' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Guardar cambios' }))

    await waitFor(() =>
      expect(desktopService.branchUpdate).toHaveBeenCalledWith({
        branchId: 'branch-1',
        expectedRevision: '2',
        name: null,
        timezone: { mode: 'SET', timezone: 'America/Cancun' },
      }),
    )
  })

  it('sends INHERIT without a timezone to remove the override', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([
      { ...branch, timezoneOverride: 'America/Cancun', effectiveTimezone: 'America/Cancun', inheritsTimezone: false },
    ])
    vi.mocked(desktopService.branchUpdate).mockResolvedValue(branch)
    renderPage()

    await chooseTimezoneMode('Heredar la del negocio')
    fireEvent.click(screen.getByRole('button', { name: 'Guardar cambios' }))

    await waitFor(() =>
      expect(desktopService.branchUpdate).toHaveBeenCalledWith({
        branchId: 'branch-1',
        expectedRevision: '2',
        name: null,
        timezone: { mode: 'INHERIT', timezone: null },
      }),
    )
  })

  it('blocks saving a SET without a timezone', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([branch])
    renderPage()

    await chooseTimezoneMode('Definir una zona propia')
    expect((screen.getByRole('button', { name: 'Guardar cambios' }) as HTMLButtonElement).disabled).toBe(true)
    expect(desktopService.branchUpdate).not.toHaveBeenCalled()
  })

  it('explains a capacity rejection from the backend', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([])
    vi.mocked(desktopService.branchCreate).mockRejectedValue(new DesktopCommandError('CONFLICT'))
    renderPage()

    fireEvent.change(await screen.findByLabelText('Código de la sucursal'), { target: { value: 'norte' } })
    fireEvent.change(screen.getByLabelText('Nombre de la sucursal'), { target: { value: 'Norte' } })
    fireEvent.click(screen.getByRole('button', { name: 'Crear sucursal' }))

    expect(await screen.findByText(/entra en conflicto con el estado actual/)).toBeDefined()
  })

  it('disables editing without branch.update', async () => {
    vi.mocked(desktopService.branchList).mockResolvedValue([branch])
    renderPage(['branch.read'])

    const name = await screen.findByLabelText('Nombre de la sucursal')
    expect((name as HTMLInputElement).disabled).toBe(true)
    expect(screen.getByText(/no modificarla/)).toBeDefined()
    expect((screen.getByRole('button', { name: 'Guardar cambios' }) as HTMLButtonElement).disabled).toBe(true)
  })
})
