// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { DesktopCatalogPage } from './DesktopCatalogPage'
import { desktopService } from '../../shared/rust-local/desktop-client'

vi.mock('../../shared/rust-local/desktop-client', () => ({
  desktopService: {
    listBranches: vi.fn(), listCategories: vi.fn(), listProducts: vi.fn(),
  },
}))
afterEach(() => { cleanup(); vi.resetAllMocks() })

function renderPage() {
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><DesktopCatalogPage /></QueryClientProvider>)
}

describe('Desktop catalog presentation (unit, service mocked)', () => {
  it('renders the typed response without converting decimal strings', async () => {
    vi.mocked(desktopService.listBranches).mockResolvedValue([{ id: 'branch', code: 'centro', name: 'Sucursal Centro', status: 'ACTIVE' }])
    vi.mocked(desktopService.listCategories).mockResolvedValue([{ id: 'category', code: 'bebidas', name: 'Bebidas', version: '0' }])
    vi.mocked(desktopService.listProducts).mockResolvedValue([{ id: 'product', sku: 'CAFE-1', name: 'Café americano', unitPrice: '12.3456', categoryId: 'category', version: '0' }])
    renderPage()
    expect(await screen.findByRole('cell', { name: '12.3456' })).toBeDefined()
    expect(screen.getByText('Sucursal Centro · centro · ACTIVE')).toBeDefined()
    expect(screen.getByText(/No productivo/)).toBeDefined()
  })

  it('shows a safe authorization failure without retrying', async () => {
    vi.mocked(desktopService.listBranches).mockResolvedValue([])
    vi.mocked(desktopService.listCategories).mockResolvedValue([])
    vi.mocked(desktopService.listProducts).mockRejectedValue(new Error('FORBIDDEN'))
    renderPage()
    expect(await screen.findByText(/No se pudo consultar el backend: FORBIDDEN/)).toBeDefined()
    expect(desktopService.listProducts).toHaveBeenCalledTimes(1)
  })
})
