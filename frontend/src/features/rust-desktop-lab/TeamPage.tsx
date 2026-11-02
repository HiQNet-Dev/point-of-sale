import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material'
import { desktopService } from '../../shared/rust-local/desktop-client'
import type { MembershipDto, PermissionDto, RoleDto, UserDto } from '../../shared/rust-local/desktop.generated'
import { describeError } from './errors'

const statusLabels: Record<string, string> = {
  ACTIVE: 'Activo',
  INACTIVE: 'Inactivo',
}

/** Contraseña mínima de un usuario nuevo; el backend aplica la misma política. */
const MIN_PASSWORD_LENGTH = 12

function invalidate(queryClient: ReturnType<typeof useQueryClient>) {
  for (const key of ['users', 'roles', 'memberships', 'auth']) {
    void queryClient.invalidateQueries({ queryKey: ['rust-desktop', key] })
  }
}

/**
 * Administración del equipo: usuarios, roles y accesos por sucursal.
 *
 * Toda la autorización se decide en Rust. La interfaz solo oculta acciones que la
 * cuenta no puede ejercer y traduce los códigos de error a mensajes legibles; no
 * envía organización, actor ni permisos al backend.
 */
export function TeamPage({
  permissions,
  currentUserId,
}: {
  permissions: string[]
  currentUserId: string
}) {
  const queryClient = useQueryClient()
  const can = (code: string) => permissions.includes(code)

  const users = useQuery({
    queryKey: ['rust-desktop', 'users'],
    queryFn: desktopService.userList,
    retry: false,
    enabled: can('user.read'),
  })
  const roles = useQuery({
    queryKey: ['rust-desktop', 'roles'],
    queryFn: desktopService.roleList,
    retry: false,
    enabled: can('role.read'),
  })
  const catalog = useQuery({
    queryKey: ['rust-desktop', 'roles', 'permissions'],
    queryFn: desktopService.permissionList,
    retry: false,
    enabled: can('permission.read'),
  })
  const memberships = useQuery({
    queryKey: ['rust-desktop', 'memberships'],
    queryFn: desktopService.membershipList,
    retry: false,
    enabled: can('branch.membership.read'),
  })

  const refresh = () => invalidate(queryClient)

  const createUser = useMutation({ mutationFn: desktopService.userCreate, onSuccess: refresh })
  const updateUser = useMutation({ mutationFn: desktopService.userUpdate, onSuccess: refresh })
  const setUserRoles = useMutation({
    mutationFn: desktopService.userSetRoles,
    onSuccess: refresh,
  })
  const createRole = useMutation({ mutationFn: desktopService.roleCreate, onSuccess: refresh })
  const setRolePermissions = useMutation({
    mutationFn: desktopService.roleSetPermissions,
    onSuccess: refresh,
  })
  const createMembership = useMutation({
    mutationFn: desktopService.membershipCreate,
    onSuccess: refresh,
  })
  const updateMembership = useMutation({
    mutationFn: desktopService.membershipUpdate,
    onSuccess: refresh,
  })

  return (
    <Stack spacing={4}>
      <Box>
        <Typography variant="h5" component="h2">Equipo</Typography>
        <Typography color="text.secondary">
          Usuarios, roles y accesos por sucursal. Los cambios de permisos se aplican en la
          siguiente operación protegida, sin reiniciar la aplicación.
        </Typography>
      </Box>

      <UsersSection
        users={users}
        roles={roles.data ?? []}
        currentUserId={currentUserId}
        canCreate={can('user.create')}
        canUpdate={can('user.update')}
        canAssignRoles={can('role.assign')}
        createPending={createUser.isPending}
        createError={createUser.isError ? describeError(createUser.error, 'write') : null}
        updatePending={updateUser.isPending}
        updateError={updateUser.isError ? describeError(updateUser.error, 'conflict') : null}
        rolesPending={setUserRoles.isPending}
        rolesError={setUserRoles.isError ? describeError(setUserRoles.error, 'write') : null}
        onCreate={(request) => createUser.mutate(request)}
        onUpdate={(request) => updateUser.mutate(request)}
        onSetRoles={(request) => setUserRoles.mutate(request)}
      />

      <Divider />

      <RolesSection
        roles={roles}
        catalog={catalog}
        canCreate={can('role.create') && can('role.permissions.manage')}
        canManagePermissions={can('role.permissions.manage')}
        createPending={createRole.isPending}
        createError={createRole.isError ? describeError(createRole.error, 'write') : null}
        permissionPending={setRolePermissions.isPending}
        permissionError={
          setRolePermissions.isError ? describeError(setRolePermissions.error, 'conflict') : null
        }
        onCreate={(request) => createRole.mutate(request)}
        onSetPermissions={(request) => setRolePermissions.mutate(request)}
      />

      <Divider />

      <MembershipsSection
        memberships={memberships}
        users={users.data ?? []}
        roles={roles.data ?? []}
        canCreate={can('branch.membership.create') && can('branch.membership.roles.manage')}
        canUpdate={can('branch.membership.update')}
        canManageRoles={can('branch.membership.roles.manage')}
        createPending={createMembership.isPending}
        createError={
          createMembership.isError ? describeError(createMembership.error, 'write') : null
        }
        updatePending={updateMembership.isPending}
        updateError={
          updateMembership.isError ? describeError(updateMembership.error, 'conflict') : null
        }
        onCreate={(request) => createMembership.mutate(request)}
        onUpdate={(request) => updateMembership.mutate(request)}
      />
    </Stack>
  )
}

interface UsersSectionProps {
  users: ReturnType<typeof useQuery<UserDto[]>>
  roles: RoleDto[]
  currentUserId: string
  canCreate: boolean
  canUpdate: boolean
  canAssignRoles: boolean
  createPending: boolean
  createError: string | null
  updatePending: boolean
  updateError: string | null
  rolesPending: boolean
  rolesError: string | null
  onCreate: (request: { displayName: string; email: string; password: string }) => void
  onUpdate: (request: {
    userId: string
    expectedRevision: string
    displayName: string | null
    email: string | null
    status: string | null
  }) => void
  onSetRoles: (request: { userId: string; roleIds: string[] }) => void
}

function UsersSection(props: UsersSectionProps) {
  const { users } = props
  return (
    <Box component="section" aria-labelledby="team-users-heading">
      <Stack spacing={2}>
        <Typography id="team-users-heading" variant="h6" component="h3">Usuarios</Typography>
        {!props.canCreate && !props.canUpdate ? (
          <Alert severity="info">Tu cuenta puede consultar el equipo, pero no modificarlo.</Alert>
        ) : null}
        {users.isPending ? <Typography role="status">Cargando usuarios…</Typography> : null}
        {users.isError ? (
          <Alert severity="error" role="alert">{describeError(users.error, 'read')}</Alert>
        ) : null}
        {users.data && users.data.length === 0 ? (
          <Alert severity="info">La organización todavía no tiene usuarios.</Alert>
        ) : null}
        {users.data && users.data.length > 0 ? (
          <Table size="small" aria-label="Usuarios de la organización">
            <TableHead>
              <TableRow>
                <TableCell>Usuario</TableCell>
                <TableCell>Roles</TableCell>
                <TableCell>Estado</TableCell>
                <TableCell>Acciones</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {users.data.map((user) => (
                <UserRow key={`${user.id}-${user.revision}`} user={user} {...props} />
              ))}
            </TableBody>
          </Table>
        ) : null}

        {props.canCreate ? (
          <NewUserForm
            pending={props.createPending}
            error={props.createError}
            onCreate={props.onCreate}
          />
        ) : null}
      </Stack>
    </Box>
  )
}

function UserRow({ user, ...props }: UsersSectionProps & { user: UserDto }) {
  const [displayName, setDisplayName] = useState(user.displayName)
  const [email, setEmail] = useState(user.email)
  const [status, setStatus] = useState(user.status)
  const [selectedRoles, setSelectedRoles] = useState<string[]>(user.roles.map((role) => role.id))
  const isSelf = props.currentUserId === user.id
  const profileDirty = displayName !== user.displayName || email !== user.email || status !== user.status
  const rolesDirty =
    selectedRoles.slice().sort().join(',') !== user.roles.map((role) => role.id).sort().join(',')

  return (
    <TableRow>
      <TableCell>
        <Stack spacing={1}>
          <TextField
            size="small"
            label="Nombre"
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            disabled={!props.canUpdate}
          />
          <TextField
            size="small"
            label="Email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            disabled={!props.canUpdate}
          />
          <Typography variant="caption" color="text.secondary">Revisión {user.revision}</Typography>
        </Stack>
      </TableCell>
      <TableCell>
        <Stack>
          {props.roles.map((role) => (
            <FormControlLabel
              key={role.id}
              control={
                <Checkbox
                  size="small"
                  checked={selectedRoles.includes(role.id)}
                  disabled={!props.canAssignRoles || isSelf}
                  slotProps={{ input: { 'aria-label': `user-role-${user.id}-${role.id}` } }}
                  onChange={(event) =>
                    setSelectedRoles((current) =>
                      event.target.checked
                        ? [...current, role.id]
                        : current.filter((id) => id !== role.id),
                    )
                  }
                />
              }
              label={`${role.name} (${role.code})`}
            />
          ))}
          {isSelf ? (
            <Typography variant="caption" color="text.secondary">
              No puedes cambiar tus propios roles.
            </Typography>
          ) : null}
        </Stack>
      </TableCell>
      <TableCell>
        <TextField
          select
          size="small"
          label="Estado"
          value={status}
          onChange={(event) => setStatus(event.target.value)}
          disabled={!props.canUpdate || isSelf}
        >
          <MenuItem value="ACTIVE">Activo</MenuItem>
          <MenuItem value="INACTIVE">Inactivo</MenuItem>
        </TextField>
      </TableCell>
      <TableCell>
        <Stack spacing={1}>
          <Button
            size="small"
            variant="contained"
            disabled={!props.canUpdate || !profileDirty || props.updatePending}
            onClick={() =>
              props.onUpdate({
                userId: user.id,
                expectedRevision: user.revision,
                displayName: displayName === user.displayName ? null : displayName,
                email: email === user.email ? null : email,
                status: status === user.status ? null : status,
              })
            }
          >
            Guardar datos
          </Button>
          <Button
            size="small"
            variant="outlined"
            disabled={!props.canAssignRoles || isSelf || !rolesDirty || props.rolesPending}
            onClick={() => props.onSetRoles({ userId: user.id, roleIds: selectedRoles })}
          >
            Guardar roles
          </Button>
        </Stack>
      </TableCell>
    </TableRow>
  )
}

function NewUserForm({
  pending,
  error,
  onCreate,
}: {
  pending: boolean
  error: string | null
  onCreate: (request: { displayName: string; email: string; password: string }) => void
}) {
  const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const valid =
    displayName.trim() !== '' &&
    email.trim() !== '' &&
    password.length >= MIN_PASSWORD_LENGTH

  return (
    <Box component="section" aria-labelledby="team-new-user-heading">
      <Stack spacing={2}>
        <Typography id="team-new-user-heading" variant="subtitle1" component="h4">
          Nuevo usuario
        </Typography>
        <TextField
          size="small"
          label="Nombre completo"
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          disabled={pending}
        />
        <TextField
          size="small"
          label="Correo del usuario"
          type="email"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          disabled={pending}
        />
        <TextField
          size="small"
          label="Contraseña inicial"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          helperText={`Mínimo ${MIN_PASSWORD_LENGTH} caracteres. No se puede volver a leer.`}
          disabled={pending}
        />
        {error ? <Alert severity="error" role="alert">{error}</Alert> : null}
        <Box>
          <Button
            variant="contained"
            disabled={!valid || pending}
            onClick={() =>
              onCreate({ displayName: displayName.trim(), email: email.trim(), password })
            }
          >
            {pending ? 'Creando…' : 'Crear usuario'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}

interface RolesSectionProps {
  roles: ReturnType<typeof useQuery<RoleDto[]>>
  catalog: ReturnType<typeof useQuery<PermissionDto[]>>
  canCreate: boolean
  canManagePermissions: boolean
  createPending: boolean
  createError: string | null
  permissionPending: boolean
  permissionError: string | null
  onCreate: (request: { code: string; name: string; permissionCodes: string[] }) => void
  onSetPermissions: (request: {
    roleId: string
    expectedRevision: string
    permissionCodes: string[]
  }) => void
}

function RolesSection({
  roles,
  catalog,
  canCreate,
  canManagePermissions,
  createPending,
  createError,
  permissionPending,
  permissionError,
  onCreate,
  onSetPermissions,
}: RolesSectionProps) {
  const [code, setCode] = useState('')
  const [name, setName] = useState('')
  const [selected, setSelected] = useState<Record<string, string[]>>({})

  return (
    <Box component="section" aria-labelledby="team-roles-heading">
      <Stack spacing={2}>
        <Typography id="team-roles-heading" variant="h6" component="h3">Roles</Typography>
        <Typography color="text.secondary">
          Un rol agrupa permisos. Solo los roles activos asignados a usuarios activos conceden
          capacidades.
        </Typography>
        {roles.isPending ? <Typography role="status">Cargando roles…</Typography> : null}
        {roles.isError ? (
          <Alert severity="error" role="alert">{describeError(roles.error, 'read')}</Alert>
        ) : null}
        {permissionError ? <Alert severity="error" role="alert">{permissionError}</Alert> : null}

        {roles.data?.map((role) => (
          <RoleEditor
            key={`${role.id}-${role.revision}`}
            role={role}
            catalog={catalog.data ?? []}
            selected={selected[role.id] ?? role.permissionCodes}
            canManage={canManagePermissions}
            pending={permissionPending}
            onChange={(codes) => setSelected((current) => ({ ...current, [role.id]: codes }))}
            onSave={() =>
              onSetPermissions({
                roleId: role.id,
                expectedRevision: role.revision,
                permissionCodes: selected[role.id] ?? role.permissionCodes,
              })
            }
          />
        ))}

        {canCreate ? (
          <Box component="section" aria-labelledby="team-new-role-heading">
            <Stack spacing={2}>
              <Typography id="team-new-role-heading" variant="subtitle1" component="h4">
                Nuevo rol
              </Typography>
              <TextField
                size="small"
                label="Código"
                value={code}
                onChange={(event) => setCode(event.target.value)}
                helperText="Identifica el rol dentro del negocio y no se puede cambiar después."
              />
              <TextField
                size="small"
                label="Nombre"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
              {createError ? <Alert severity="error" role="alert">{createError}</Alert> : null}
              <Box>
                <Button
                  variant="contained"
                  disabled={createPending || code.trim() === '' || name.trim() === ''}
                  onClick={() =>
                    onCreate({ code: code.trim(), name: name.trim(), permissionCodes: [] })
                  }
                >
                  {createPending ? 'Creando…' : 'Crear rol'}
                </Button>
              </Box>
            </Stack>
          </Box>
        ) : null}
      </Stack>
    </Box>
  )
}

function RoleEditor({
  role,
  catalog,
  selected,
  canManage,
  pending,
  onChange,
  onSave,
}: {
  role: RoleDto
  catalog: PermissionDto[]
  selected: string[]
  canManage: boolean
  pending: boolean
  onChange: (codes: string[]) => void
  onSave: () => void
}) {
  return (
    <Box component="section" aria-labelledby={`role-${role.id}-heading`}>
      <Stack spacing={1}>
        <Typography id={`role-${role.id}-heading`} variant="subtitle1" component="h4">
          {role.name} · {role.code} · {statusLabels[role.status] ?? role.status}
        </Typography>
        {catalog.length === 0 ? (
          <Typography variant="caption" color="text.secondary">
            Sin catálogo de permisos disponible para esta cuenta.
          </Typography>
        ) : (
          <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))' }}>
            {catalog.map((permission) => (
              <FormControlLabel
                key={permission.code}
                control={
                  <Checkbox
                    size="small"
                    checked={selected.includes(permission.code)}
                    disabled={!canManage}
                    slotProps={{ input: { 'aria-label': `role-permission-${role.id}-${permission.code}` } }}
                    onChange={(event) =>
                      onChange(
                        event.target.checked
                          ? [...selected, permission.code]
                          : selected.filter((code) => code !== permission.code),
                      )
                    }
                  />
                }
                label={permission.code}
              />
            ))}
          </Box>
        )}
        <Box>
          <Button size="small" variant="outlined" disabled={!canManage || pending} onClick={onSave}>
            {pending ? 'Guardando…' : 'Guardar permisos'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}

interface MembershipsSectionProps {
  memberships: ReturnType<typeof useQuery<MembershipDto[]>>
  users: UserDto[]
  roles: RoleDto[]
  canCreate: boolean
  canUpdate: boolean
  canManageRoles: boolean
  createPending: boolean
  createError: string | null
  updatePending: boolean
  updateError: string | null
  onCreate: (request: { branchId: string; userId: string; roleIds: string[] }) => void
  onUpdate: (request: {
    membershipId: string
    expectedRevision: string
    status: string | null
    roleIds: string[] | null
  }) => void
}

function MembershipsSection(props: MembershipsSectionProps) {
  const [userId, setUserId] = useState('')
  const [branchId, setBranchId] = useState('')

  return (
    <Box component="section" aria-labelledby="team-memberships-heading">
      <Stack spacing={2}>
        <Typography id="team-memberships-heading" variant="h6" component="h3">
          Accesos por sucursal
        </Typography>
        <Typography color="text.secondary">
          Sin una membresía activa con rol activo, una cuenta no puede operar en la sucursal
          aunque tenga permisos de organización.
        </Typography>
        {props.memberships.isPending ? (
          <Typography role="status">Cargando accesos…</Typography>
        ) : null}
        {props.memberships.isError ? (
          <Alert severity="error" role="alert">{describeError(props.memberships.error, 'read')}</Alert>
        ) : null}
        {props.updateError ? <Alert severity="error" role="alert">{props.updateError}</Alert> : null}

        {props.memberships.data?.map((membership) => (
          <MembershipRow
            key={`${membership.id}-${membership.revision}`}
            membership={membership}
            users={props.users}
            roles={props.roles}
            canUpdate={props.canUpdate}
            canManageRoles={props.canManageRoles}
            pending={props.updatePending}
            onUpdate={props.onUpdate}
          />
        ))}

        {props.canCreate ? (
          <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} sx={{ alignItems: 'flex-start' }}>
            <TextField
              select
              size="small"
              label="Usuario"
              value={userId}
              onChange={(event) => setUserId(event.target.value)}
              sx={{ minWidth: 220 }}
            >
              {props.users.map((user) => (
                <MenuItem key={user.id} value={user.id}>{user.email}</MenuItem>
              ))}
            </TextField>
            <TextField
              size="small"
              label="Sucursal (UUID)"
              value={branchId}
              onChange={(event) => setBranchId(event.target.value)}
              sx={{ minWidth: 260 }}
            />
            {props.createError ? <Alert severity="error" role="alert">{props.createError}</Alert> : null}
            <Button
              variant="contained"
              disabled={props.createPending || userId === '' || branchId.trim() === ''}
              onClick={() =>
                props.onCreate({ branchId: branchId.trim(), userId, roleIds: [] })
              }
            >
              {props.createPending ? 'Asignando…' : 'Dar acceso'}
            </Button>
          </Stack>
        ) : null}
      </Stack>
    </Box>
  )
}

function MembershipRow({
  membership,
  users,
  roles,
  canUpdate,
  canManageRoles,
  pending,
  onUpdate,
}: {
  membership: MembershipDto
  users: UserDto[]
  roles: RoleDto[]
  canUpdate: boolean
  canManageRoles: boolean
  pending: boolean
  onUpdate: (request: {
    membershipId: string
    expectedRevision: string
    status: string | null
    roleIds: string[] | null
  }) => void
}) {
  const [selected, setSelected] = useState<string[]>(membership.roles.map((role) => role.id))
  const [status, setStatus] = useState(membership.status)
  const email = users.find((user) => user.id === membership.userId)?.email ?? membership.userId
  const dirty =
    selected.slice().sort().join(',') !== membership.roles.map((role) => role.id).sort().join(',')

  return (
    <Box component="section" aria-labelledby={`membership-${membership.id}-heading`}>
      <Typography id={`membership-${membership.id}-heading`} variant="subtitle1" component="h4">
        {email} · sucursal {membership.branchId}
      </Typography>
      <Stack spacing={1} sx={{ mt: 1 }}>
          <Stack direction="row" spacing={2} sx={{ flexWrap: 'wrap' }}>
          {roles.map((role) => (
            <FormControlLabel
              key={role.id}
              control={
                <Checkbox
                  size="small"
                  checked={selected.includes(role.id)}
                  disabled={!canManageRoles}
                  slotProps={{ input: { 'aria-label': `membership-role-${membership.id}-${role.id}` } }}
                  onChange={(event) =>
                    setSelected((current) =>
                      event.target.checked
                        ? [...current, role.id]
                        : current.filter((id) => id !== role.id),
                    )
                  }
                />
              }
              label={`${role.name} (${role.code})`}
            />
          ))}
        </Stack>
          <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
          <TextField
            select
            size="small"
            label="Estado"
            value={status}
            onChange={(event) => setStatus(event.target.value)}
            disabled={!canUpdate}
          >
            <MenuItem value="ACTIVE">Activo</MenuItem>
            <MenuItem value="INACTIVE">Inactivo</MenuItem>
          </TextField>
          <Button
            size="small"
            variant="outlined"
            disabled={!canUpdate || pending || (!dirty && status === membership.status)}
            onClick={() =>
              onUpdate({
                membershipId: membership.id,
                expectedRevision: membership.revision,
                status: status === membership.status ? null : status,
                roleIds: dirty ? selected : null,
              })
            }
          >
            Guardar acceso
          </Button>
          <Typography variant="caption" color="text.secondary">
            Revisión {membership.revision}
          </Typography>
        </Stack>
      </Stack>
    </Box>
  )
}
