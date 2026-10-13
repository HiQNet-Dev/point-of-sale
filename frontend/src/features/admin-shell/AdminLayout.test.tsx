// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AuthSessionContext } from '../auth/model/authSessionContext'
import type { AuthSessionValue } from '../auth/model/authSessionContext'
import type { AuthMe } from '../auth/api/authApi'
import { AdminLayout } from './AdminLayout'

function authValue(): AuthSessionValue {
  const me: AuthMe = {
    userId: 'user-1',
    organizationId: 'org-1',
    email: 'admin@example.com',
    status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
  return { status: 'AUTHENTICATED', me, logoutNotice: null, login: async () => me, logout: async () => true }
}

function renderLayout() {
  render(
    <AuthSessionContext.Provider value={authValue()}>
      <MemoryRouter initialEntries={['/admin/inventory/items']}>
        <Routes>
          <Route path="/admin" element={<AdminLayout />}>
            <Route path="inventory/items" element={<div>items-outlet</div>} />
          </Route>
        </Routes>
      </MemoryRouter>
    </AuthSessionContext.Provider>,
  )
}

afterEach(() => {
  cleanup()
})

describe('AdminLayout', () => {
  it('exposes navigation, session affordances and the admin content area', () => {
    renderLayout()

    expect(screen.getByRole('link', { name: 'Artículos' })).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Existencias por sucursal' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Abrir navegación' })).toBeTruthy()
    expect(screen.getByRole('link', { name: 'Punto de venta' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Salir' })).toBeTruthy()
    expect(screen.getByText('admin@example.com')).toBeTruthy()
    expect(screen.getByText('items-outlet')).toBeTruthy()
  })

  it('links the inventory recipe and modifier effect sections without unfinished entries', () => {
    renderLayout()

    expect(screen.getByRole('link', { name: 'Recetas' }).getAttribute('href')).toBe('/admin/inventory/recipes')
    expect(screen.getByRole('link', { name: 'Efectos de modificadores' }).getAttribute('href')).toBe(
      '/admin/inventory/modifier-effects',
    )
    expect(screen.queryByRole('link', { name: /stock negativo|política de stock/i })).toBeNull()
  })

  it('links the organization inventory settings section', () => {
    renderLayout()

    expect(screen.getByRole('link', { name: 'Configuración' }).getAttribute('href')).toBe(
      '/admin/inventory/settings',
    )
  })
})
