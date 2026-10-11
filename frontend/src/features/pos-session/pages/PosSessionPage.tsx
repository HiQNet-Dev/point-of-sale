import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import CircularProgress from '@mui/material/CircularProgress'
import Container from '@mui/material/Container'
import FormControl from '@mui/material/FormControl'
import InputLabel from '@mui/material/InputLabel'
import MenuItem from '@mui/material/MenuItem'
import Paper from '@mui/material/Paper'
import Select from '@mui/material/Select'
import Stack from '@mui/material/Stack'
import Typography from '@mui/material/Typography'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { useAuthSession } from '../../auth/model/useAuthSession'
import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'
import { AmountDialog } from '../components/AmountDialog'
import {
  closeCashShift,
  listCashRegisters,
  listMyBranches,
  listOpenCashShifts,
  openCashShift,
} from '../api/posSessionApi'
import type { Branch, CashRegister, CashShift } from '../api/posSessionApi'
import { formatMoney, formatTimestamp } from '../model/moneyInput'
import { posSessionKeys } from '../model/posSessionKeys'

const PAGE_SIZE = 20
const EMPTY_BRANCHES: Branch[] = []
const EMPTY_REGISTERS: CashRegister[] = []

type DialogMode = 'open' | 'close' | null

function isPermissionError(error: unknown): boolean {
  return error instanceof ApiRequestError && error.status === 403
}

function isNotFound(error: unknown): boolean {
  return error instanceof ApiRequestError && error.status === 404
}

function safeErrorMessage(error: unknown): string {
  if (error instanceof ApiNetworkError) return 'No se pudo conectar con el servidor. Revisa tu conexión.'
  if (error instanceof ApiRequestError && error.status >= 500) return 'El servicio no está disponible temporalmente.'
  return 'No se pudo completar la operación. Actualiza los datos e inténtalo de nuevo.'
}

function StatusChip({ status }: { status: 'ACTIVE' | 'INACTIVE' }) {
  return <Chip size="small" label={status === 'ACTIVE' ? 'Activa' : 'Inactiva'} color={status === 'ACTIVE' ? 'success' : 'default'} />
}

function PaginationControls({
  page,
  totalPages,
  onChange,
  label,
}: {
  page: number
  totalPages: number
  onChange(page: number): void
  label: string
}) {
  if (totalPages <= 1) return null
  return (
    <Stack direction="row" spacing={2} sx={{ alignItems: 'center', justifyContent: 'space-between' }}>
      <Typography variant="body2" color="text.secondary">Página {page + 1} de {totalPages}</Typography>
      <Stack direction="row" spacing={1}>
        <Button aria-label={`Página anterior de ${label}`} disabled={page === 0} onClick={() => onChange(page - 1)}>
          Anterior
        </Button>
        <Button aria-label={`Página siguiente de ${label}`} disabled={page + 1 >= totalPages} onClick={() => onChange(page + 1)}>
          Siguiente
        </Button>
      </Stack>
    </Stack>
  )
}

function LoadingState({ label }: { label: string }) {
  return (
    <Stack role="status" aria-live="polite" direction="row" spacing={1.5} sx={{ py: 2, alignItems: 'center' }}>
      <CircularProgress size={20} aria-label={`Cargando ${label}`} />
      <Typography color="text.secondary">Cargando {label}…</Typography>
    </Stack>
  )
}

export function PosSessionPage() {
  const { me } = useAuthSession()
  const queryClient = useQueryClient()
  const [branchPage, setBranchPage] = useState(0)
  const [registerPage, setRegisterPage] = useState(0)
  const [selectedBranchId, setSelectedBranchId] = useState('')
  const [selectedRegisterId, setSelectedRegisterId] = useState('')
  const [suppressRegisterAutoSelect, setSuppressRegisterAutoSelect] = useState(false)
  const [dialogMode, setDialogMode] = useState<DialogMode>(null)
  const [message, setMessage] = useState<string | null>(null)

  const branchesQuery = useQuery({
    queryKey: posSessionKeys.branches(branchPage, PAGE_SIZE),
    queryFn: () => listMyBranches({ page: branchPage, size: PAGE_SIZE }),
    enabled: Boolean(me),
  })
  const branches = branchesQuery.data?.items ?? EMPTY_BRANCHES
  const selectedBranchIdForScope = useMemo(() => {
    if (branches.some((branch) => branch.id === selectedBranchId)) return selectedBranchId
    if (!selectedBranchId && branchesQuery.isSuccess && branchesQuery.data.totalElements === 1 && branches.length === 1) {
      return branches[0].id
    }
    return ''
  }, [branches, branchesQuery.data, branchesQuery.isSuccess, selectedBranchId])
  const selectedBranch = branches.find((branch) => branch.id === selectedBranchIdForScope)

  const registersQuery = useQuery({
    queryKey: posSessionKeys.registers(me?.organizationId ?? '', selectedBranchIdForScope, registerPage, PAGE_SIZE),
    queryFn: () => listCashRegisters(me!.organizationId, selectedBranchIdForScope, { page: registerPage, size: PAGE_SIZE }),
    enabled: Boolean(me && selectedBranchIdForScope),
  })
  const registers = registersQuery.data?.items ?? EMPTY_REGISTERS
  const canAutoSelectRegister = !suppressRegisterAutoSelect && !isNotFound(registersQuery.error)
  const selectedRegisterIdForScope = useMemo(() => {
    if (isNotFound(registersQuery.error)) return ''
    if (registers.some((register) => register.id === selectedRegisterId)) return selectedRegisterId
    if (
      canAutoSelectRegister
      && !selectedRegisterId
      && registersQuery.isSuccess
      && registersQuery.data.totalElements === 1
      && registers.length === 1
    ) {
      return registers[0].id
    }
    return ''
  }, [canAutoSelectRegister, registers, registersQuery.data, registersQuery.error, registersQuery.isSuccess, selectedRegisterId])
  const selectedRegister = registers.find((register) => register.id === selectedRegisterIdForScope)

  const shiftsQuery = useQuery({
    queryKey: posSessionKeys.openShifts(me?.organizationId ?? '', selectedBranchIdForScope, selectedRegisterIdForScope),
    queryFn: () => listOpenCashShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
    enabled: Boolean(me && selectedBranchIdForScope && selectedRegisterIdForScope),
  })
  const openShifts = shiftsQuery.data?.items ?? []
  const invariantBroken = (shiftsQuery.data?.totalElements ?? 0) > 1 || openShifts.length > 1
  const activeShift = !invariantBroken && openShifts.length === 1 ? openShifts[0] : undefined

  const branchSelection = (id: string) => {
    setSelectedBranchId(id)
    setSelectedRegisterId('')
    setSuppressRegisterAutoSelect(false)
    setRegisterPage(0)
    setDialogMode(null)
    setMessage(null)
  }

  const registerSelection = (id: string) => {
    setSelectedRegisterId(id)
    setSuppressRegisterAutoSelect(false)
    setDialogMode(null)
    setMessage(null)
  }

  const openMutation = useMutation({
    mutationFn: (openingAmount: string) => openCashShift(
      me!.organizationId,
      selectedBranchIdForScope,
      selectedRegisterIdForScope,
      openingAmount,
    ),
  })
  const closeMutation = useMutation({
    mutationFn: ({ shift, amount }: { shift: CashShift; amount: string }) => closeCashShift(
      me!.organizationId,
      selectedBranchIdForScope,
      selectedRegisterIdForScope,
      shift.id,
      shift.version,
      amount,
    ),
  })

  async function submitOpen(amount: string) {
    try {
      await openMutation.mutateAsync(amount)
      setDialogMode(null)
      setMessage(null)
      await queryClient.invalidateQueries({
        queryKey: posSessionKeys.openShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
      })
    } catch (error) {
      if (error instanceof ApiRequestError && error.code === 'CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT') {
        setDialogMode(null)
        setMessage('La caja ya tiene un turno activo. Se actualizará su estado.')
        await queryClient.invalidateQueries({
          queryKey: posSessionKeys.openShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
        })
        return
      }
      if (error instanceof ApiRequestError && error.code === 'CASH_REGISTER_INACTIVE') {
        setDialogMode(null)
        setMessage('Esta caja está inactiva. Se actualizará su estado.')
        await queryClient.invalidateQueries({
          queryKey: posSessionKeys.registers(me!.organizationId, selectedBranchIdForScope, registerPage, PAGE_SIZE),
        })
        return
      }
      if (error instanceof ApiRequestError && error.code === 'BRANCH_INACTIVE') {
        setDialogMode(null)
        setMessage('Esta sucursal está inactiva. Se actualizará su estado.')
        await queryClient.invalidateQueries({ queryKey: posSessionKeys.branches(branchPage, PAGE_SIZE) })
        return
      }
      if (isNotFound(error)) {
        setDialogMode(null)
        setSelectedRegisterId('')
        setSuppressRegisterAutoSelect(true)
        setMessage('La sucursal o la caja ya no están disponibles. Se actualizarán los datos.')
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: posSessionKeys.branches(branchPage, PAGE_SIZE) }),
          queryClient.invalidateQueries({
            queryKey: posSessionKeys.registers(me!.organizationId, selectedBranchIdForScope, registerPage, PAGE_SIZE),
          }),
        ])
        return
      }
      setMessage(isPermissionError(error) ? 'No tienes permiso para abrir turnos en esta caja.' : safeErrorMessage(error))
      throw error
    }
  }

  async function submitClose(amount: string) {
    if (!activeShift) return
    try {
      await closeMutation.mutateAsync({ shift: activeShift, amount })
      setDialogMode(null)
      setMessage(null)
      await queryClient.invalidateQueries({
        queryKey: posSessionKeys.openShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
      })
    } catch (error) {
      if (error instanceof ApiRequestError && error.code === 'CONCURRENT_MODIFICATION') {
        setDialogMode(null)
        setMessage('El turno cambió mientras intentabas cerrarlo. Se actualizarán los datos.')
        await queryClient.invalidateQueries({
          queryKey: posSessionKeys.openShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
        })
        return
      }
      if (error instanceof ApiRequestError && error.code === 'CASH_SHIFT_ALREADY_CLOSED') {
        setDialogMode(null)
        setMessage('El turno ya fue cerrado. Se actualizarán los datos.')
        await queryClient.invalidateQueries({
          queryKey: posSessionKeys.openShifts(me!.organizationId, selectedBranchIdForScope, selectedRegisterIdForScope),
        })
        return
      }
      if (isNotFound(error)) {
        setDialogMode(null)
        setSelectedRegisterId('')
        setSuppressRegisterAutoSelect(true)
        setRegisterPage(0)
        setMessage('La caja o el turno ya no están disponibles. Selecciona una caja actualizada.')
        await queryClient.invalidateQueries({
          queryKey: posSessionKeys.registers(me!.organizationId, selectedBranchIdForScope, registerPage, PAGE_SIZE),
        })
        return
      }
      setMessage(isPermissionError(error) ? 'No tienes permiso para cerrar este turno.' : safeErrorMessage(error))
      throw error
    }
  }

  if (!me) return null

  const shiftError = shiftsQuery.error
  const registerError = registersQuery.error
  const branchError = branchesQuery.error
  const selectedTimezone = selectedBranch?.timezone || me.organization.timezone
  const canOpen = Boolean(
    selectedBranch?.status === 'ACTIVE'
    && selectedRegister?.status === 'ACTIVE'
    && shiftsQuery.isSuccess
    && !invariantBroken
    && !activeShift,
  )

  return (
    <Box component="main" sx={{ minHeight: '100dvh', py: { xs: 3, md: 6 }, px: 2, bgcolor: 'background.default' }}>
      <Container maxWidth="md" disableGutters>
        <Stack spacing={3}>
          <Box>
            <Typography variant="overline" color="primary.main" sx={{ fontWeight: 700 }}>HiQNet POS</Typography>
            <Typography component="h1" variant="h4">Punto de venta</Typography>
            <Typography color="text.secondary">Hola, {me.email}</Typography>
          </Box>

          {message && <Alert role="status" severity="info" onClose={() => setMessage(null)}>{message}</Alert>}

          {branchError && (
            <Alert
              severity={isPermissionError(branchError) ? 'warning' : 'error'}
              role="alert"
              action={<Button color="inherit" onClick={() => void branchesQuery.refetch()}>Reintentar</Button>}
            >
              {isPermissionError(branchError)
                ? 'No se pudieron consultar tus sucursales.'
                : isNotFound(branchError)
                  ? 'El contexto de sucursales ya no está disponible.'
                  : safeErrorMessage(branchError)}
            </Alert>
          )}

          <Paper variant="outlined" sx={{ p: { xs: 2, sm: 3 } }}>
            <Stack spacing={3}>
              <Typography component="h2" variant="h6">Contexto operacional</Typography>

              {branchesQuery.isLoading ? <LoadingState label="sucursales" /> : null}
              {branchesQuery.isSuccess && branchesQuery.data.totalElements === 0 && (
                <Alert severity="info">No tienes sucursales asignadas.</Alert>
              )}
              {branchesQuery.isSuccess && branchesQuery.data.totalElements > 0 && (
                <>
                  <FormControl fullWidth>
                    <InputLabel id="branch-select-label">Sucursal</InputLabel>
                    <Select
                      labelId="branch-select-label"
                      id="branch-select"
                      label="Sucursal"
                      value={selectedBranchIdForScope}
                      onChange={(event) => branchSelection(event.target.value)}
                      displayEmpty
                      renderValue={(id) => {
                        const branch = branches.find((item) => item.id === id)
                        return branch ? `${branch.name} · ${branch.code}` : 'Selecciona una sucursal'
                      }}
                    >
                      <MenuItem value="" disabled>Selecciona una sucursal</MenuItem>
                      {branches.map((branch) => (
                        <MenuItem key={branch.id} value={branch.id}>
                          <Stack direction="row" spacing={1.5} sx={{ width: '100%', alignItems: 'center', justifyContent: 'space-between' }}>
                            <span>{branch.name} · {branch.code}</span>
                            <StatusChip status={branch.status} />
                          </Stack>
                        </MenuItem>
                      ))}
                    </Select>
                  </FormControl>
                  {selectedBranchId && !selectedBranch && branchesQuery.isSuccess && (
                    <Alert severity="warning">La sucursal seleccionada ya no está disponible. Elige una sucursal actualizada.</Alert>
                  )}
                  {selectedBranch && (
                    <Stack direction="row" spacing={1.5} sx={{ alignItems: 'center' }}>
                      <Typography variant="body2">Sucursal actual: <strong>{selectedBranch.name}</strong> ({selectedBranch.code})</Typography>
                      <StatusChip status={selectedBranch.status} />
                    </Stack>
                  )}
                  <PaginationControls
                    page={branchPage}
                    totalPages={branchesQuery.data.totalPages}
                    label="sucursales"
                    onChange={(page) => {
                      setBranchPage(page)
                      setSelectedBranchId('')
                      setSelectedRegisterId('')
                      setSuppressRegisterAutoSelect(false)
                      setRegisterPage(0)
                      setDialogMode(null)
                    }}
                  />
                </>
              )}

              {selectedBranchIdForScope && (
                <>
                  <Box>
                    <Typography component="h3" variant="subtitle1" sx={{ mb: 1.5 }}>Caja</Typography>
                    {registersQuery.isLoading && <LoadingState label="cajas" />}
                    {registerError && (
                      <Alert
                        severity={isPermissionError(registerError) ? 'warning' : 'error'}
                        role="alert"
                        action={isNotFound(registerError)
                          ? <Button color="inherit" onClick={() => void registersQuery.refetch()}>Reintentar</Button>
                          : undefined}
                      >
                        {isPermissionError(registerError)
                          ? 'No tienes permiso para consultar las cajas de esta sucursal.'
                          : isNotFound(registerError)
                            ? 'La sucursal o sus cajas ya no están disponibles.'
                            : safeErrorMessage(registerError)}
                      </Alert>
                    )}
                    {registersQuery.isSuccess && registersQuery.data.totalElements === 0 && (
                      <Alert severity="info">Esta sucursal no tiene cajas disponibles.</Alert>
                    )}
                    {registersQuery.isSuccess && registersQuery.data.totalElements > 0 && (
                      <Stack spacing={2}>
                        <FormControl fullWidth>
                          <InputLabel id="register-select-label">Caja</InputLabel>
                          <Select
                            labelId="register-select-label"
                            id="register-select"
                            label="Caja"
                            value={selectedRegisterIdForScope}
                            onChange={(event) => registerSelection(event.target.value)}
                            displayEmpty
                            renderValue={(id) => {
                              const register = registers.find((item) => item.id === id)
                              return register ? `${register.name} · ${register.code}` : 'Selecciona una caja'
                            }}
                          >
                            <MenuItem value="" disabled>Selecciona una caja</MenuItem>
                            {registers.map((register) => (
                              <MenuItem key={register.id} value={register.id}>
                                <Stack direction="row" spacing={1.5} sx={{ width: '100%', alignItems: 'center', justifyContent: 'space-between' }}>
                                  <span>{register.name} · {register.code}</span>
                                  <StatusChip status={register.status} />
                                </Stack>
                              </MenuItem>
                            ))}
                          </Select>
                        </FormControl>
                        {selectedRegisterId && !selectedRegister && registersQuery.isSuccess && (
                          <Alert severity="warning">La caja seleccionada ya no está disponible. Elige una caja actualizada.</Alert>
                        )}
                        {selectedRegister && (
                          <Stack direction="row" spacing={1.5} sx={{ alignItems: 'center' }}>
                            <Typography variant="body2">Caja actual: <strong>{selectedRegister.name}</strong> ({selectedRegister.code})</Typography>
                            <StatusChip status={selectedRegister.status} />
                          </Stack>
                        )}
                        <PaginationControls
                          page={registerPage}
                          totalPages={registersQuery.data.totalPages}
                          label="cajas"
                          onChange={(page) => {
                            setRegisterPage(page)
                            setSelectedRegisterId('')
                            setSuppressRegisterAutoSelect(false)
                            setDialogMode(null)
                          }}
                        />
                      </Stack>
                    )}
                  </Box>
                </>
              )}

              {selectedRegisterIdForScope && selectedBranch && selectedRegister && (
                <Box>
                  <Typography component="h3" variant="subtitle1" sx={{ mb: 1.5 }}>Turno</Typography>
                  {shiftsQuery.isLoading && <LoadingState label="turno" />}
                  {shiftError && (
                    <Alert
                      severity={isPermissionError(shiftError) ? 'warning' : 'error'}
                      role="alert"
                      action={isNotFound(shiftError)
                        ? <Button
                            color="inherit"
                            onClick={() => {
                              setSelectedRegisterId('')
                              setSuppressRegisterAutoSelect(true)
                              setMessage('La caja ya no está disponible. Selecciona una caja actualizada.')
                            }}
                          >
                            Elegir otra caja
                          </Button>
                        : undefined}
                    >
                      {isPermissionError(shiftError)
                        ? 'No tienes permiso para consultar los turnos de esta caja.'
                        : isNotFound(shiftError)
                          ? 'La caja ya no está disponible.'
                          : safeErrorMessage(shiftError)}
                    </Alert>
                  )}
                  {shiftsQuery.isSuccess && invariantBroken && (
                    <Alert severity="error" role="alert">
                      Hay más de un turno OPEN para esta caja. No se seleccionó ninguno; solicita asistencia.
                    </Alert>
                  )}
                  {shiftsQuery.isSuccess && !invariantBroken && !activeShift && (
                    <Stack spacing={2}>
                      {selectedBranch.status === 'INACTIVE' && (
                        <Alert severity="warning">Esta sucursal está inactiva. No se pueden abrir nuevos turnos.</Alert>
                      )}
                      {selectedBranch.status === 'ACTIVE' && selectedRegister.status === 'INACTIVE' && (
                        <Alert severity="warning">Esta caja está inactiva. No se pueden abrir nuevos turnos.</Alert>
                      )}
                      <Stack direction="row" sx={{ alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 2 }}>
                        <Box>
                          <Typography variant="overline" color="text.secondary">ESTADO DE TURNO</Typography>
                          <Typography component="h4" variant="h6">Sin turno activo</Typography>
                        </Box>
                        <Button variant="contained" disabled={!canOpen} onClick={() => setDialogMode('open')}>
                          Abrir turno
                        </Button>
                      </Stack>
                    </Stack>
                  )}
                  {shiftsQuery.isSuccess && activeShift && (
                    <Paper variant="outlined" sx={{ p: { xs: 2, sm: 3 }, bgcolor: 'background.paper' }}>
                      <Stack spacing={2.5}>
                        <Stack direction="row" sx={{ alignItems: 'center', justifyContent: 'space-between', gap: 2 }}>
                          <Box>
                            <Typography variant="overline" color="text.secondary">TURNO ACTIVO</Typography>
                            <Typography component="h4" variant="h6">En curso</Typography>
                          </Box>
                          <Chip label="OPEN · Activo" color="success" />
                        </Stack>
                        <Box component="dl" sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'minmax(150px, 1fr) 2fr' }, gap: 1, m: 0 }}>
                          <Typography component="dt" variant="body2" color="text.secondary">Sucursal</Typography>
                          <Typography component="dd" variant="body2" sx={{ m: 0 }}>{selectedBranch.name} · {selectedBranch.code}</Typography>
                          <Typography component="dt" variant="body2" color="text.secondary">Caja</Typography>
                          <Typography component="dd" variant="body2" sx={{ m: 0 }}>{selectedRegister.name} · {selectedRegister.code}</Typography>
                          <Typography component="dt" variant="body2" color="text.secondary">Fondo inicial</Typography>
                          <Typography component="dd" variant="body2" sx={{ m: 0 }}>
                            {formatMoney(activeShift.openingAmount, me.organization.locale, me.organization.currency)}
                          </Typography>
                          <Typography component="dt" variant="body2" color="text.secondary">Apertura</Typography>
                          <Typography component="dd" variant="body2" sx={{ m: 0 }}>
                            {formatTimestamp(activeShift.openedAt, me.organization.locale, selectedTimezone)}
                          </Typography>
                        </Box>
                        <Box>
                          <Button variant="contained" color="primary" onClick={() => setDialogMode('close')}>
                            Cerrar turno
                          </Button>
                        </Box>
                      </Stack>
                    </Paper>
                  )}
                </Box>
              )}
            </Stack>
          </Paper>
        </Stack>
      </Container>

      <AmountDialog
        open={dialogMode !== null}
        mode={dialogMode ?? 'open'}
        pending={dialogMode === 'open' ? openMutation.isPending : closeMutation.isPending}
        onClose={() => setDialogMode(null)}
        onConfirm={dialogMode === 'close' ? submitClose : submitOpen}
      />
    </Box>
  )
}
