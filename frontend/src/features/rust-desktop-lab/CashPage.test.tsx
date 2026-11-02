// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { CashPage } from './CashPage'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'

vi.mock('../../shared/rust-local/desktop-client', async () => {
  const actual = await vi.importActual<typeof import('../../shared/rust-local/desktop-client')>(
    '../../shared/rust-local/desktop-client',
  )
  return {
    DesktopCommandError: actual.DesktopCommandError,
    desktopService: {
      cashRegisterList: vi.fn(),
      cashRegisterCreate: vi.fn(),
      cashRegisterUpdate: vi.fn(),
      cashShiftOpen: vi.fn(),
      cashShiftGetOpen: vi.fn(),
      cashShiftList: vi.fn(),
      cashShiftClose: vi.fn(),
    },
  }
})

afterEach(() => {
  cleanup()
  vi.resetAllMocks()
})

const permissions = [
  'cash-register.read',
  'cash-register.create',
  'cash-register.update',
  'cash-shift.open',
  'cash-shift.read',
  'cash-shift.close',
]

const register = {
  id: 'register-1',
  branchId: 'branch-1',
  code: 'caja-1',
  name: 'Caja principal',
  status: 'ACTIVE',
  revision: '0',
}

const openShift = {
  id: 'shift-1',
  cashRegisterId: 'register-1',
  status: 'OPEN',
  openingAmount: '150.0000',
  closingDeclaredAmount: null,
  openedAt: '1793235600',
  closedAt: null,
  openedByUserId: 'user-admin',
  closedByUserId: null,
  revision: '0',
}

function renderPage(branchSelected = true) {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <CashPage permissions={permissions} branchSelected={branchSelected} />
    </QueryClientProvider>,
  )
}

function mockEmpty() {
  vi.mocked(desktopService.cashRegisterList).mockResolvedValue([])
  vi.mocked(desktopService.cashShiftList).mockResolvedValue([])
  vi.mocked(desktopService.cashShiftGetOpen).mockResolvedValue(null)
}

describe('Cash module (unit, service mocked)', () => {
  it('explains that a branch must be selected before touching cash', async () => {
    renderPage(false)
    expect(screen.getByText(/Selecciona una sucursal/)).toBeDefined()
    expect(desktopService.cashRegisterList).not.toHaveBeenCalled()
  })

  it('lists the registers of the selected branch', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterList).mockResolvedValue([register])
    renderPage()

    expect(await screen.findByText('Caja principal')).toBeDefined()
    expect(screen.getByText('caja-1')).toBeDefined()
    expect(screen.getByText('Activa')).toBeDefined()
  })

  it('creates a register with code and name only', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterCreate).mockResolvedValue(register)
    renderPage()

    fireEvent.change(await screen.findByLabelText('Código de la caja'), {
      target: { value: 'caja-1' },
    })
    fireEvent.change(screen.getByLabelText('Nombre de la caja'), {
      target: { value: 'Caja principal' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Crear caja' }))

    await waitFor(() => expect(desktopService.cashRegisterCreate).toHaveBeenCalled())
    const [request] = vi.mocked(desktopService.cashRegisterCreate).mock.calls[0]
    expect(request).toEqual({ code: 'caja-1', name: 'Caja principal' })
  })

  it('opens a shift with the exact amount string', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterList).mockResolvedValue([register])
    vi.mocked(desktopService.cashShiftOpen).mockResolvedValue(openShift)
    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'Administrar' }))
    fireEvent.change(await screen.findByLabelText('Importe inicial'), {
      target: { value: '150.0000' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Abrir turno' }))

    await waitFor(() => expect(desktopService.cashShiftOpen).toHaveBeenCalled())
    const [request] = vi.mocked(desktopService.cashShiftOpen).mock.calls[0]
    expect(request).toEqual({ cashRegisterId: 'register-1', openingAmount: '150.0000' })
  })

  it('shows the open shift and closes it with the revision read', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterList).mockResolvedValue([register])
    vi.mocked(desktopService.cashShiftGetOpen).mockResolvedValue(openShift)
    vi.mocked(desktopService.cashShiftClose).mockResolvedValue({
      ...openShift,
      status: 'CLOSED',
      closingDeclaredAmount: '150.0000',
      closedAt: '1793239200',
      closedByUserId: 'user-admin',
      revision: '1',
    })
    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'Administrar' }))
    expect(await screen.findByText('Turno abierto')).toBeDefined()
    expect(screen.getByText('150.0000')).toBeDefined()

    fireEvent.change(screen.getByLabelText('Importe declarado al cierre'), {
      target: { value: '150.0000' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Cerrar turno' }))

    await waitFor(() => expect(desktopService.cashShiftClose).toHaveBeenCalled())
    const [request] = vi.mocked(desktopService.cashShiftClose).mock.calls[0]
    expect(request).toEqual({
      cashRegisterId: 'register-1',
      cashShiftId: 'shift-1',
      expectedRevision: '0',
      closingDeclaredAmount: '150.0000',
    })
  })

  it('surfaces a close conflict with an actionable message', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterList).mockResolvedValue([register])
    vi.mocked(desktopService.cashShiftGetOpen).mockResolvedValue(openShift)
    vi.mocked(desktopService.cashShiftClose).mockRejectedValue(
      new DesktopCommandError('CONFLICT'),
    )
    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'Administrar' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Cerrar turno' }))

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('conflicto')
    expect(screen.getByText(/El cierre se rechaza si quedan ventas abiertas/)).toBeDefined()
  })

  it('rejects an amount with more than four decimals before calling Rust', async () => {
    mockEmpty()
    vi.mocked(desktopService.cashRegisterList).mockResolvedValue([register])
    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'Administrar' }))
    fireEvent.change(await screen.findByLabelText('Importe inicial'), {
      target: { value: '10.12345' },
    })

    const openButton = screen.getByRole('button', { name: 'Abrir turno' }) as HTMLButtonElement
    expect(openButton.disabled).toBe(true)
  })
})
