import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  Alert,
  Box,
  Button,
  Divider,
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
import type { CashRegisterDto, CashShiftDto } from '../../shared/rust-local/desktop.generated'
import { describeError } from './errors'

const statusLabels: Record<string, string> = {
  ACTIVE: 'Activa',
  INACTIVE: 'Inactiva',
  OPEN: 'Abierto',
  CLOSED: 'Cerrado',
}

/** Importe decimal con hasta cuatro decimales, como texto exacto. */
const AMOUNT_PATTERN = /^\d{1,12}(\.\d{1,4})?$/

const HISTORY_PAGE_SIZE = 20

function isAmount(value: string): boolean {
  return AMOUNT_PATTERN.test(value.trim())
}

/** Instante Unix en segundos a texto local legible. */
function formatInstant(seconds: string | null | undefined): string {
  if (!seconds) return '—'
  const value = Number(seconds)
  if (!Number.isFinite(value)) return '—'
  return new Date(value * 1000).toLocaleString()
}

/**
 * Módulo operativo de caja.
 *
 * La sucursal sale del scope verificado de la sesión: aquí no se elige ni se
 * envía. Si no hay sucursal seleccionada, el backend deniega y la interfaz lo
 * explica en lugar de inventar un contexto.
 */
export function CashPage({
  permissions,
  branchSelected,
}: {
  permissions: string[]
  branchSelected: boolean
}) {
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const can = (code: string) => permissions.includes(code)

  const registers = useQuery({
    queryKey: ['rust-desktop', 'cash', 'registers'],
    queryFn: () => desktopService.cashRegisterList({ status: null, limit: 100, offset: 0 }),
    retry: false,
    enabled: branchSelected && can('cash-register.read'),
  })

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'cash'] })
  }

  const createRegister = useMutation({ mutationFn: desktopService.cashRegisterCreate, onSuccess: refresh })
  const updateRegister = useMutation({ mutationFn: desktopService.cashRegisterUpdate, onSuccess: refresh })
  const openShift = useMutation({ mutationFn: desktopService.cashShiftOpen, onSuccess: refresh })
  const closeShift = useMutation({ mutationFn: desktopService.cashShiftClose, onSuccess: refresh })

  const selected = registers.data?.find((register) => register.id === selectedId) ?? null

  if (!branchSelected) {
    return (
      <Alert severity="info">
        Selecciona una sucursal en «Operación» para administrar sus cajas y turnos.
      </Alert>
    )
  }
  if (!can('cash-register.read')) {
    return <Alert severity="info">Tu cuenta no tiene permiso para consultar las cajas de la sucursal.</Alert>
  }
  if (registers.isPending) {
    return <Typography role="status">Cargando cajas…</Typography>
  }
  if (registers.isError) {
    return <Alert severity="error" role="alert">{describeError(registers.error, 'read')}</Alert>
  }

  return (
    <Stack spacing={4}>
      <Box>
        <Typography variant="h5" component="h2">Caja</Typography>
        <Typography color="text.secondary">
          Cajas de la sucursal seleccionada y sus turnos. Un turno se abre sobre una caja
          activa y es la unidad operativa de la jornada.
        </Typography>
      </Box>

      {registers.data.length === 0 ? (
        <Alert severity="info">La sucursal todavía no tiene cajas configuradas.</Alert>
      ) : (
        <Table size="small" aria-label="Cajas de la sucursal">
          <TableHead>
            <TableRow>
              <TableCell>Nombre</TableCell>
              <TableCell>Código</TableCell>
              <TableCell>Estado</TableCell>
              <TableCell>Acción</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {registers.data.map((register) => (
              <TableRow key={`${register.id}-${register.revision}`} selected={register.id === selectedId}>
                <TableCell>{register.name}</TableCell>
                <TableCell>{register.code}</TableCell>
                <TableCell>{statusLabels[register.status] ?? register.status}</TableCell>
                <TableCell>
                  <Button size="small" onClick={() => setSelectedId(register.id)}>
                    Administrar
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {can('cash-register.create') ? (
        <NewRegisterForm
          pending={createRegister.isPending}
          error={createRegister.isError ? describeError(createRegister.error, 'write') : null}
          onCreate={(request) => createRegister.mutate(request)}
        />
      ) : null}

      {selected ? (
        <>
          <Divider />
          <RegisterDetail
            register={selected}
            canUpdate={can('cash-register.update')}
            canOpen={can('cash-shift.open')}
            canClose={can('cash-shift.close')}
            canReadShifts={can('cash-shift.read')}
            updatePending={updateRegister.isPending}
            updateError={updateRegister.isError ? describeError(updateRegister.error, 'conflict') : null}
            openPending={openShift.isPending}
            openError={openShift.isError ? describeError(openShift.error, 'write') : null}
            closePending={closeShift.isPending}
            closeError={closeShift.isError ? describeError(closeShift.error, 'write') : null}
            onUpdate={(request) => updateRegister.mutate(request)}
            onOpen={(request) => openShift.mutate(request)}
            onClose={(request) => closeShift.mutate(request)}
          />
        </>
      ) : null}
    </Stack>
  )
}

function NewRegisterForm({
  pending,
  error,
  onCreate,
}: {
  pending: boolean
  error: string | null
  onCreate: (request: { code: string; name: string }) => void
}) {
  const [code, setCode] = useState('')
  const [name, setName] = useState('')

  return (
    <Box component="section" aria-labelledby="cash-new-register-heading">
      <Stack spacing={2}>
        <Typography id="cash-new-register-heading" variant="subtitle1" component="h3">
          Nueva caja
        </Typography>
        <TextField
          label="Código de la caja"
          value={code}
          onChange={(event) => setCode(event.target.value)}
          helperText="Identifica la caja dentro de la sucursal y no se puede cambiar después."
          disabled={pending}
        />
        <TextField
          label="Nombre de la caja"
          value={name}
          onChange={(event) => setName(event.target.value)}
          disabled={pending}
        />
        {error ? <Alert severity="error" role="alert">{error}</Alert> : null}
        <Box>
          <Button
            variant="contained"
            disabled={pending || code.trim() === '' || name.trim() === ''}
            onClick={() => onCreate({ code: code.trim(), name: name.trim() })}
          >
            {pending ? 'Creando…' : 'Crear caja'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}

interface RegisterDetailProps {
  register: CashRegisterDto
  canUpdate: boolean
  canOpen: boolean
  canClose: boolean
  canReadShifts: boolean
  updatePending: boolean
  updateError: string | null
  openPending: boolean
  openError: string | null
  closePending: boolean
  closeError: string | null
  onUpdate: (request: {
    cashRegisterId: string
    expectedRevision: string
    name: string | null
    status: string | null
  }) => void
  onOpen: (request: { cashRegisterId: string; openingAmount: string }) => void
  onClose: (request: {
    cashRegisterId: string
    cashShiftId: string
    expectedRevision: string
    closingDeclaredAmount: string
  }) => void
}

function RegisterDetail({
  register,
  canUpdate,
  canOpen,
  canClose,
  canReadShifts,
  updatePending,
  updateError,
  openPending,
  openError,
  closePending,
  closeError,
  onUpdate,
  onOpen,
  onClose,
}: RegisterDetailProps) {
  const queryClient = useQueryClient()
  const [name, setName] = useState(register.name)
  const [status, setStatus] = useState(register.status)

  const openShiftQuery = useQuery({
    queryKey: ['rust-desktop', 'cash', 'open', register.id],
    queryFn: () => desktopService.cashShiftGetOpen(register.id),
    retry: false,
    enabled: canReadShifts,
  })
  const history = useQuery({
    queryKey: ['rust-desktop', 'cash', 'history', register.id],
    queryFn: () =>
      desktopService.cashShiftList({
        cashRegisterId: register.id,
        status: null,
        limit: HISTORY_PAGE_SIZE,
        offset: 0,
      }),
    retry: false,
    enabled: canReadShifts,
  })

  const dirty = name !== register.name || status !== register.status
  const open = openShiftQuery.data ?? null

  return (
    <Box component="section" aria-labelledby="cash-register-heading">
      <Stack spacing={3}>
        <Box>
          <Typography id="cash-register-heading" variant="h6" component="h3">
            {register.name} · {register.code}
          </Typography>
          <Typography color="text.secondary">
            {statusLabels[register.status] ?? register.status} · revisión {register.revision}
          </Typography>
        </Box>

        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} sx={{ alignItems: 'flex-start' }}>
          <TextField
            label="Nombre de la caja"
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={!canUpdate || updatePending}
          />
          <TextField
            select
            label="Estado de la caja"
            value={status}
            onChange={(event) => setStatus(event.target.value)}
            disabled={!canUpdate || updatePending}
            sx={{ minWidth: 180 }}
          >
            <MenuItem value="ACTIVE">Activa</MenuItem>
            <MenuItem value="INACTIVE">Inactiva</MenuItem>
          </TextField>
          <Button
            variant="contained"
            disabled={!canUpdate || !dirty || updatePending}
            onClick={() =>
              onUpdate({
                cashRegisterId: register.id,
                expectedRevision: register.revision,
                name: name === register.name ? null : name,
                status: status === register.status ? null : status,
              })
            }
          >
            {updatePending ? 'Guardando…' : 'Guardar caja'}
          </Button>
        </Stack>
        {updateError ? <Alert severity="error" role="alert">{updateError}</Alert> : null}

        {openShiftQuery.isError ? (
          <Alert severity="error" role="alert">{describeError(openShiftQuery.error, 'read')}</Alert>
        ) : null}

        {open ? (
          <OpenShiftPanel
            shift={open}
            canClose={canClose}
            pending={closePending}
            error={closeError}
            onClose={(amount) =>
              onClose({
                cashRegisterId: register.id,
                cashShiftId: open.id,
                expectedRevision: open.revision,
                closingDeclaredAmount: amount,
              })
            }
          />
        ) : (
          <OpenShiftForm
            canOpen={canOpen}
            pending={openPending}
            error={openError}
            onOpen={(amount) => onOpen({ cashRegisterId: register.id, openingAmount: amount })}
          />
        )}

        <Divider />

        <Box>
          <Typography variant="subtitle1" component="h4">Historial de turnos</Typography>
          {!canReadShifts ? (
            <Typography variant="caption" color="text.secondary">
              Tu cuenta no puede consultar el historial de turnos.
            </Typography>
          ) : history.isPending ? (
            <Typography role="status">Cargando historial…</Typography>
          ) : history.isError ? (
            <Alert severity="error" role="alert">{describeError(history.error, 'read')}</Alert>
          ) : history.data.length === 0 ? (
            <Typography variant="caption" color="text.secondary">
              Esta caja todavía no tiene turnos.
            </Typography>
          ) : (
            <Table size="small" aria-label={`Historial de turnos de ${register.code}`}>
              <TableHead>
                <TableRow>
                  <TableCell>Apertura</TableCell>
                  <TableCell>Cierre</TableCell>
                  <TableCell>Inicial</TableCell>
                  <TableCell>Declarado</TableCell>
                  <TableCell>Estado</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {history.data.map((shift: CashShiftDto) => (
                  <TableRow key={shift.id}>
                    <TableCell>{formatInstant(shift.openedAt)}</TableCell>
                    <TableCell>{formatInstant(shift.closedAt)}</TableCell>
                    <TableCell>{shift.openingAmount}</TableCell>
                    <TableCell>{shift.closingDeclaredAmount ?? '—'}</TableCell>
                    <TableCell>{statusLabels[shift.status] ?? shift.status}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </Box>
      </Stack>
      <Button
        size="small"
        sx={{ mt: 2 }}
        onClick={() => void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'cash'] })}
      >
        Actualizar turnos
      </Button>
    </Box>
  )
}

function OpenShiftForm({
  canOpen,
  pending,
  error,
  onOpen,
}: {
  canOpen: boolean
  pending: boolean
  error: string | null
  onOpen: (amount: string) => void
}) {
  const [amount, setAmount] = useState('0.0000')

  return (
    <Box component="section" aria-labelledby="cash-open-heading">
      <Stack spacing={2}>
        <Typography id="cash-open-heading" variant="subtitle1" component="h4">
          Abrir turno
        </Typography>
        <TextField
          label="Importe inicial"
          value={amount}
          onChange={(event) => setAmount(event.target.value)}
          helperText="Efectivo declarado al abrir el turno, hasta cuatro decimales."
          error={amount.trim() !== '' && !isAmount(amount)}
          disabled={!canOpen || pending}
        />
        {!canOpen ? (
          <Alert severity="info">Tu cuenta puede consultar la caja, pero no abrir turnos.</Alert>
        ) : null}
        {error ? <Alert severity="error" role="alert">{error}</Alert> : null}
        <Box>
          <Button
            variant="contained"
            disabled={!canOpen || pending || !isAmount(amount)}
            onClick={() => onOpen(amount.trim())}
          >
            {pending ? 'Abriendo…' : 'Abrir turno'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}

function OpenShiftPanel({
  shift,
  canClose,
  pending,
  error,
  onClose,
}: {
  shift: CashShiftDto
  canClose: boolean
  pending: boolean
  error: string | null
  onClose: (amount: string) => void
}) {
  const [amount, setAmount] = useState(shift.openingAmount)

  return (
    <Box component="section" aria-labelledby="cash-shift-heading">
      <Stack spacing={2}>
        <Typography id="cash-shift-heading" variant="subtitle1" component="h4">
          Turno abierto
        </Typography>
        <Table size="small" aria-label="Turno abierto">
          <TableBody>
            <TableRow><TableCell>Operador</TableCell><TableCell>{shift.openedByUserId}</TableCell></TableRow>
            <TableRow><TableCell>Apertura</TableCell><TableCell>{formatInstant(shift.openedAt)}</TableCell></TableRow>
            <TableRow><TableCell>Importe inicial</TableCell><TableCell>{shift.openingAmount}</TableCell></TableRow>
            <TableRow><TableCell>Estado</TableCell><TableCell>Abierto</TableCell></TableRow>
            <TableRow><TableCell>Revisión</TableCell><TableCell>{shift.revision}</TableCell></TableRow>
          </TableBody>
        </Table>
        <TextField
          label="Importe declarado al cierre"
          value={amount}
          onChange={(event) => setAmount(event.target.value)}
          helperText="Efectivo contado por el operador, hasta cuatro decimales."
          error={amount.trim() !== '' && !isAmount(amount)}
          disabled={!canClose || pending}
        />
        {!canClose ? (
          <Alert severity="info">Tu cuenta puede consultar el turno, pero no cerrarlo.</Alert>
        ) : null}
        {error ? <Alert severity="error" role="alert">{error}</Alert> : null}
        <Typography variant="caption" color="text.secondary">
          El cierre se rechaza si quedan ventas abiertas, si la caja o la sucursal se
          desactivaron sin capacidad de recuperación, o si otra sesión cambió la revisión.
        </Typography>
        <Box>
          <Button
            variant="contained"
            disabled={!canClose || pending || !isAmount(amount)}
            onClick={() => onClose(amount.trim())}
          >
            {pending ? 'Cerrando…' : 'Cerrar turno'}
          </Button>
        </Box>
      </Stack>
    </Box>
  )
}
