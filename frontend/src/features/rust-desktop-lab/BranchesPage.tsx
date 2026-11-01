import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, MenuItem, Stack, Table, TableBody, TableCell, TableRow, TextField, Typography } from '@mui/material'
import { desktopService } from '../../shared/rust-local/desktop-client'
import type { BranchAdminDto, UpdateBranchRequestDto_Deserialize } from '../../shared/rust-local/desktop.generated'
import { describeError } from './errors'

const statusLabels: Record<string, string> = {
  ACTIVE: 'Operativo',
  INACTIVE: 'Inactivo',
}

type TimezoneMode = 'KEEP' | 'SET' | 'INHERIT'

const timezoneModes: { value: TimezoneMode; label: string; hint: string }[] = [
  { value: 'KEEP', label: 'Conservar la configuración actual', hint: 'No cambia la zona horaria.' },
  { value: 'SET', label: 'Definir una zona propia', hint: 'La sucursal deja de heredar la zona del negocio.' },
  { value: 'INHERIT', label: 'Heredar la del negocio', hint: 'Se elimina la zona propia de la sucursal.' },
]

/**
 * Administración de la sucursal del negocio.
 *
 * Free Local opera con una única sucursal: si la instalación todavía no tiene
 * ninguna, aquí se configura la inicial. La segunda alta la rechaza el backend
 * —la capacidad vive en la capa de aplicación— aunque la interfaz no la ofrezca.
 */
export function BranchesPage({ permissions }: { permissions: string[] }) {
  const queryClient = useQueryClient()
  const branches = useQuery({
    queryKey: ['rust-desktop', 'branches', 'admin'],
    queryFn: desktopService.branchList,
    retry: false,
  })
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'branches'] })

  const create = useMutation({
    mutationFn: (request: { code: string; name: string; timezone: string | null }) =>
      desktopService.branchCreate(request),
    onSuccess: refresh,
  })
  const update = useMutation({
    mutationFn: (request: UpdateBranchRequestDto_Deserialize) =>
      desktopService.branchUpdate(request),
    onSuccess: refresh,
  })

  if (branches.isPending) {
    return <Typography role="status">Cargando sucursales…</Typography>
  }
  if (branches.isError) {
    return <Alert severity="error" role="alert">{describeError(branches.error, 'read')}</Alert>
  }
  if (branches.data.length === 0) {
    return (
      <InitialBranchForm
        canCreate={permissions.includes('branch.create')}
        pending={create.isPending}
        error={create.isError ? describeError(create.error, 'write') : null}
        onCreate={(request) => create.mutate(request)}
      />
    )
  }

  return (
    <Stack spacing={3}>
      <Box>
        <Typography variant="h5" component="h2">Sucursales</Typography>
        <Typography color="text.secondary">
          Administración de la sucursal del negocio. La selección operativa se hace en la
          pestaña «Operación».
        </Typography>
      </Box>
      {update.isError ? (
        <Alert severity="error" role="alert">{describeError(update.error, 'conflict')}</Alert>
      ) : null}
      {update.isSuccess ? <Alert severity="success">Sucursal actualizada.</Alert> : null}
      {branches.data.map((branch) => (
        <BranchSettings
          key={`${branch.id}-${branch.revision}`}
          branch={branch}
          canUpdate={permissions.includes('branch.update')}
          pending={update.isPending}
          onSave={(request) => update.mutate(request)}
        />
      ))}
      <Alert severity="info">
        Free Local admite una única sucursal. La creación de sucursales adicionales
        pertenece a otra edición del producto.
      </Alert>
    </Stack>
  )
}

interface InitialBranchFormProps {
  canCreate: boolean
  pending: boolean
  error: string | null
  onCreate: (request: { code: string; name: string; timezone: string | null }) => void
}

function InitialBranchForm({ canCreate, pending, error, onCreate }: InitialBranchFormProps) {
  const [code, setCode] = useState('')
  const [name, setName] = useState('')
  const [timezone, setTimezone] = useState('')

  return (
    <Box component="section" aria-labelledby="initial-branch-heading">
      <Stack spacing={3}>
        <Box>
          <Typography id="initial-branch-heading" variant="h5" component="h2">
            Sucursal inicial
          </Typography>
          <Typography color="text.secondary">
            Esta instalación todavía no tiene sucursal. Configura la primera para poder operar.
          </Typography>
        </Box>
        <TextField
          label="Código de la sucursal"
          value={code}
          onChange={(event) => setCode(event.target.value)}
          disabled={!canCreate || pending}
          helperText="Identifica la sucursal dentro del negocio y no se puede cambiar después."
          fullWidth
        />
        <TextField
          label="Nombre de la sucursal"
          value={name}
          onChange={(event) => setName(event.target.value)}
          disabled={!canCreate || pending}
          fullWidth
        />
        <TextField
          label="Zona horaria propia (opcional)"
          value={timezone}
          onChange={(event) => setTimezone(event.target.value)}
          disabled={!canCreate || pending}
          helperText="Si la dejas vacía, la sucursal hereda la zona horaria del negocio."
          fullWidth
        />
        {!canCreate ? (
          <Alert severity="info">Tu cuenta no tiene permiso para configurar la sucursal.</Alert>
        ) : null}
        {error ? <Alert severity="error" role="alert">{error}</Alert> : null}
        <Box>
          <Button
            variant="contained"
            onClick={() =>
              onCreate({
                code: code.trim(),
                name: name.trim(),
                timezone: timezone.trim() === '' ? null : timezone.trim(),
              })
            }
            disabled={!canCreate || pending || code.trim() === '' || name.trim() === ''}
          >
            {pending ? 'Creando…' : 'Crear sucursal'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}

interface BranchFormState {
  name: string
  mode: TimezoneMode
  timezone: string
}

function toBranchForm(branch: BranchAdminDto): BranchFormState {
  return { name: branch.name, mode: 'KEEP', timezone: branch.timezoneOverride ?? '' }
}

interface BranchSettingsProps {
  branch: BranchAdminDto
  canUpdate: boolean
  pending: boolean
  onSave: (request: UpdateBranchRequestDto_Deserialize) => void
}

function BranchSettings({ branch, canUpdate, pending, onSave }: BranchSettingsProps) {
  const [form, setForm] = useState<BranchFormState>(() => toBranchForm(branch))
  const dirty = form.name !== branch.name || form.mode !== 'KEEP'
  const canSubmit =
    canUpdate && !pending && dirty && (form.mode !== 'SET' || form.timezone.trim() !== '')

  return (
    <Box component="section" aria-labelledby={`branch-${branch.id}-heading`}>
      <Stack spacing={3}>
        <Box>
          <Typography id={`branch-${branch.id}-heading`} variant="h6" component="h3">
            {branch.name}
          </Typography>
          <Typography color="text.secondary">
            {branch.code} · {statusLabels[branch.status] ?? branch.status}
          </Typography>
        </Box>

        <Table aria-label={`Datos de la sucursal ${branch.code}`}>
          <TableBody>
            <TableRow>
              <TableCell>Código</TableCell>
              <TableCell>{branch.code} (no editable)</TableCell>
            </TableRow>
            <TableRow>
              <TableCell>Zona horaria</TableCell>
              <TableCell>
                {branch.effectiveTimezone} ·{' '}
                {branch.inheritsTimezone ? 'heredada del negocio' : 'propia de la sucursal'}
              </TableCell>
            </TableRow>
            <TableRow>
              <TableCell>Estado</TableCell>
              <TableCell>{statusLabels[branch.status] ?? branch.status} (no editable)</TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <TextField
          label="Nombre de la sucursal"
          value={form.name}
          onChange={(event) => setForm({ ...form, name: event.target.value })}
          disabled={!canUpdate || pending}
          fullWidth
        />
        <TextField
          select
          label="Zona horaria"
          value={form.mode}
          onChange={(event) => setForm({ ...form, mode: event.target.value as TimezoneMode })}
          disabled={!canUpdate || pending}
          helperText={timezoneModes.find((mode) => mode.value === form.mode)?.hint}
          fullWidth
        >
          {timezoneModes.map((mode) => (
            <MenuItem key={mode.value} value={mode.value}>{mode.label}</MenuItem>
          ))}
        </TextField>
        {form.mode === 'SET' ? (
          <TextField
            label="Zona horaria propia"
            value={form.timezone}
            onChange={(event) => setForm({ ...form, timezone: event.target.value })}
            disabled={!canUpdate || pending}
            helperText="Zona IANA, por ejemplo America/Cancun."
            fullWidth
          />
        ) : null}

        {!canUpdate ? (
          <Alert severity="info">Tu cuenta puede consultar la sucursal, pero no modificarla.</Alert>
        ) : null}

        <Stack direction="row" spacing={2}>
          <Button
            variant="contained"
            onClick={() =>
              onSave({
                branchId: branch.id,
                expectedRevision: branch.revision,
                name: form.name === branch.name ? null : form.name,
                timezone: {
                  mode: form.mode,
                  timezone: form.mode === 'SET' ? form.timezone.trim() : null,
                },
              })
            }
            disabled={!canSubmit}
          >
            {pending ? 'Guardando…' : 'Guardar cambios'}
          </Button>
          <Button
            variant="outlined"
            onClick={() => setForm(toBranchForm(branch))}
            disabled={pending || !dirty}
          >
            Descartar cambios
          </Button>
        </Stack>
        <Typography variant="caption" color="text.secondary">Revisión {branch.revision}</Typography>
      </Stack>
    </Box>
  )
}
