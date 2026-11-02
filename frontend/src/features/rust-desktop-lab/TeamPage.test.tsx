// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TeamPage } from './TeamPage'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'

vi.mock('../../shared/rust-local/desktop-client', async () => {
  const actual = await vi.importActual<typeof import('../../shared/rust-local/desktop-client')>(
    '../../shared/rust-local/desktop-client',
  )
  return {
    DesktopCommandError: actual.DesktopCommandError,
    desktopService: {
      userList: vi.fn(),
      roleList: vi.fn(),
      permissionList: vi.fn(),
      membershipList: vi.fn(),
      userCreate: vi.fn(),
      userUpdate: vi.fn(),
      userSetRoles: vi.fn(),
      roleCreate: vi.fn(),
      roleSetPermissions: vi.fn(),
      membershipCreate: vi.fn(),
      membershipUpdate: vi.fn(),
    },
  }
})

afterEach(() => {
  cleanup()
  vi.resetAllMocks()
})

const permissions = [
  'user.read',
  'user.create',
  'user.update',
  'role.read',
  'role.create',
  'role.update',
  'role.assign',
  'role.permissions.manage',
  'permission.read',
  'branch.membership.read',
  'branch.membership.create',
  'branch.membership.update',
  'branch.membership.roles.manage',
]

const admin = {
  id: 'user-admin',
  displayName: 'Administradora',
  email: 'admin@hiqnet.test',
  status: 'ACTIVE',
  revision: '0',
  roles: [{ id: 'role-admin', code: 'administrador', name: 'Administrador', status: 'ACTIVE', revision: '0' }],
}

const cashier = {
  id: 'user-cashier',
  displayName: 'Cajera',
  email: 'cajera@hiqnet.test',
  status: 'ACTIVE',
  revision: '1',
  roles: [],
}

const roles = [
  { id: 'role-admin', code: 'administrador', name: 'Administrador', status: 'ACTIVE', revision: '2', permissionCodes: ['user.read'] },
  { id: 'role-cashier', code: 'cajero', name: 'Cajero', status: 'ACTIVE', revision: '0', permissionCodes: [] },
]

const catalog = [
  { code: 'user.read', description: 'Read users' },
  { code: 'cash-shift.open', description: 'Open shift' },
]

function renderPage(granted: string[] = permissions) {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <TeamPage permissions={granted} currentUserId="user-admin" />
    </QueryClientProvider>,
  )
}

describe('Team administration (unit, service mocked)', () => {
  it('renders users and roles read from Rust', async () => {
    vi.mocked(desktopService.userList).mockResolvedValue([admin, cashier])
    vi.mocked(desktopService.roleList).mockResolvedValue(roles)
    vi.mocked(desktopService.permissionList).mockResolvedValue(catalog)
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    renderPage()

    expect(await screen.findByDisplayValue('Cajera')).toBeDefined()
    expect(screen.getByDisplayValue('cajera@hiqnet.test')).toBeDefined()
    expect(screen.getAllByText(/Administrador/).length).toBeGreaterThan(0)
  })

  it('prevents changing your own organisation roles', async () => {
    vi.mocked(desktopService.userList).mockResolvedValue([admin])
    vi.mocked(desktopService.roleList).mockResolvedValue(roles)
    vi.mocked(desktopService.permissionList).mockResolvedValue(catalog)
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    renderPage()

    expect(await screen.findByText(/No puedes cambiar tus propios roles/)).toBeDefined()
    const saveRoles = screen.getAllByRole('button', { name: 'Guardar roles' })[0] as HTMLButtonElement
    expect(saveRoles.disabled).toBe(true)
  })

  it('creates a user with the typed request and no invented fields', async () => {
    vi.mocked(desktopService.userList).mockResolvedValue([])
    vi.mocked(desktopService.roleList).mockResolvedValue(roles)
    vi.mocked(desktopService.permissionList).mockResolvedValue(catalog)
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    vi.mocked(desktopService.userCreate).mockResolvedValue(cashier)
    renderPage()

    fireEvent.change(await screen.findByLabelText('Nombre completo'), {
      target: { value: 'Cajera' },
    })
    fireEvent.change(screen.getByLabelText('Correo del usuario'), {
      target: { value: 'cajera@hiqnet.test' },
    })
    fireEvent.change(screen.getByLabelText('Contraseña inicial'), {
      target: { value: 'clave-de-cajera-larga' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Crear usuario' }))

    await waitFor(() => expect(desktopService.userCreate).toHaveBeenCalled())
    const [createRequest] = vi.mocked(desktopService.userCreate).mock.calls[0]
    expect(createRequest).toEqual({
      displayName: 'Cajera',
      email: 'cajera@hiqnet.test',
      password: 'clave-de-cajera-larga',
    })
  })

  it('assigns organisation roles with the exact selected identifiers', async () => {
    vi.mocked(desktopService.userList).mockResolvedValue([admin, cashier])
    vi.mocked(desktopService.roleList).mockResolvedValue(roles)
    vi.mocked(desktopService.permissionList).mockResolvedValue(catalog)
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    vi.mocked(desktopService.userSetRoles).mockResolvedValue({
      ...cashier,
      roles: [roles[1]],
    })
    renderPage()

    const checkboxes = await screen.findAllByLabelText('user-role-user-cashier-role-cashier')
    fireEvent.click(checkboxes[0])
    const saveRoles = screen.getAllByRole('button', { name: 'Guardar roles' })[1]
    fireEvent.click(saveRoles)

    await waitFor(() => expect(desktopService.userSetRoles).toHaveBeenCalled())
    const [rolesRequest] = vi.mocked(desktopService.userSetRoles).mock.calls[0]
    expect(rolesRequest).toEqual({ userId: 'user-cashier', roleIds: ['role-cashier'] })
  })

  it('reports a forbidden response without inventing a cause', async () => {
    vi.mocked(desktopService.userList).mockRejectedValue(new DesktopCommandError('FORBIDDEN'))
    vi.mocked(desktopService.roleList).mockResolvedValue([])
    vi.mocked(desktopService.permissionList).mockResolvedValue([])
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    renderPage()

    expect(await screen.findByRole('alert')).toHaveProperty(
      'textContent',
      'La cuenta no tiene permiso para esta operación.',
    )
  })

  it('saves role permissions using the exact revision string', async () => {
    vi.mocked(desktopService.userList).mockResolvedValue([])
    vi.mocked(desktopService.roleList).mockResolvedValue(roles)
    vi.mocked(desktopService.permissionList).mockResolvedValue(catalog)
    vi.mocked(desktopService.membershipList).mockResolvedValue([])
    vi.mocked(desktopService.roleSetPermissions).mockResolvedValue(roles[0])
    renderPage()

    const cashShift = await screen.findByLabelText('role-permission-role-admin-cash-shift.open')
    fireEvent.click(cashShift)
    const saveButtons = screen.getAllByRole('button', { name: 'Guardar permisos' })
    // El primer editor corresponde al rol `administrador` (revisión 2).
    fireEvent.click(saveButtons[0])

    await waitFor(() => expect(desktopService.roleSetPermissions).toHaveBeenCalled())
    const [permissionsRequest] = vi.mocked(desktopService.roleSetPermissions).mock.calls[0]
    expect(permissionsRequest.roleId).toBe('role-admin')
    expect(permissionsRequest.expectedRevision).toBe('2')
    expect(permissionsRequest.permissionCodes.slice().sort()).toEqual([
      'cash-shift.open',
      'user.read',
    ])
  })
})
