import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import CircularProgress from '@mui/material/CircularProgress'
import Chip from '@mui/material/Chip'
import FormControl from '@mui/material/FormControl'
import InputLabel from '@mui/material/InputLabel'
import MenuItem from '@mui/material/MenuItem'
import Paper from '@mui/material/Paper'
import Select from '@mui/material/Select'
import Stack from '@mui/material/Stack'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuthSession } from '../../auth/model/useAuthSession'
import { listMyBranches } from '../../pos-session/api/posSessionApi'
import { BASE_UNIT_LABELS, INVENTORY_BASE_UNITS, INVENTORY_ITEM_STATUSES, ITEM_STATUS_LABELS } from '../../inventory-items/api/inventoryItemsApi'
import type { InventoryBaseUnit, InventoryItemStatus } from '../../inventory-items/api/inventoryItemsApi'
import { INVENTORY_STOCK_PAGE_SIZE, listBranchStock } from '../api/inventoryStockApi'
import type { BranchStockListQuery } from '../api/inventoryStockApi'
import { inventoryStockKeys } from '../model/inventoryStockKeys'
import { inventoryStockErrorMessage } from '../model/inventoryStockErrors'
import { formatQuantity } from '../model/inventoryQuantity'

export function BranchStockPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''
  const [searchParams, setSearchParams] = useSearchParams()
  const branchId = searchParams.get('branch') ?? ''

  const [qInput, setQInput] = useState('')
  const [q, setQ] = useState('')
  const [status, setStatus] = useState<InventoryItemStatus | ''>('')
  const [baseUnit, setBaseUnit] = useState<InventoryBaseUnit | ''>('')
  const [page, setPage] = useState(0)

  const branchesQuery = useQuery({
    queryKey: ['self', 'branches', organizationId],
    queryFn: () => listMyBranches({ page: 0, size: 100 }),
    enabled: Boolean(organizationId),
  })

  const query: BranchStockListQuery = {
    page,
    size: INVENTORY_STOCK_PAGE_SIZE,
    q,
    status: status || undefined,
    baseUnit: baseUnit || undefined,
  }

  const stockQuery = useQuery({
    queryKey: inventoryStockKeys.list(organizationId, branchId, query),
    queryFn: () => listBranchStock(organizationId, branchId, query),
    enabled: Boolean(organizationId) && Boolean(branchId),
    placeholderData: (previous) => previous,
  })

  const branches = branchesQuery.data?.items ?? []
  const rows = stockQuery.data?.items ?? []
  const totalElements = stockQuery.data?.totalElements ?? 0
  const totalPages = stockQuery.data?.totalPages ?? 0

  function selectBranch(value: string) {
    setPage(0)
    setSearchParams(value ? { branch: value } : {})
  }

  return (
    <Stack spacing={2}>
      <Box>
        <Typography component="h2" variant="h5">Existencias por sucursal</Typography>
        <Typography variant="body2" color="text.secondary">
          La existencia pertenece a la sucursal. Selecciona una sucursal para consultar y registrar movimientos.
        </Typography>
      </Box>

      <Paper variant="outlined" sx={{ p: 2 }}>
        <Stack spacing={2}>
          {branchesQuery.isSuccess && branches.length === 0 && (
            <Alert severity="info" role="alert">No tienes sucursales asignadas.</Alert>
          )}
          {branchesQuery.isError && (
            <Alert severity="error" role="alert">{inventoryStockErrorMessage(branchesQuery.error)}</Alert>
          )}
          <FormControl sx={{ minWidth: 240, maxWidth: 360 }}>
            <InputLabel id="stock-branch-label">Sucursal</InputLabel>
            <Select
              labelId="stock-branch-label"
              label="Sucursal"
              value={branchId}
              onChange={(event) => selectBranch(event.target.value)}
            >
              <MenuItem value=""><em>Selecciona una sucursal</em></MenuItem>
              {branches.map((branch) => (
                <MenuItem key={branch.id} value={branch.id}>{branch.name} · {branch.code}</MenuItem>
              ))}
            </Select>
          </FormControl>

          {branchId && (
            <Stack
              direction={{ xs: 'column', md: 'row' }}
              spacing={2}
              component="form"
              onSubmit={(event) => {
                event.preventDefault()
                setQ(qInput)
                setPage(0)
              }}
            >
              <TextField
                label="Buscar"
                value={qInput}
                onChange={(event) => setQInput(event.target.value)}
                placeholder="Código o nombre"
                sx={{ flexGrow: 1 }}
              />
              <FormControl sx={{ minWidth: 160 }}>
                <InputLabel id="stock-status-label">Estado</InputLabel>
                <Select
                  labelId="stock-status-label"
                  label="Estado"
                  value={status}
                  onChange={(event) => {
                    setStatus(event.target.value as InventoryItemStatus | '')
                    setPage(0)
                  }}
                >
                  <MenuItem value="">Todos</MenuItem>
                  {INVENTORY_ITEM_STATUSES.map((value) => (
                    <MenuItem key={value} value={value}>{ITEM_STATUS_LABELS[value]}</MenuItem>
                  ))}
                </Select>
              </FormControl>
              <FormControl sx={{ minWidth: 160 }}>
                <InputLabel id="stock-unit-label">Unidad base</InputLabel>
                <Select
                  labelId="stock-unit-label"
                  label="Unidad base"
                  value={baseUnit}
                  onChange={(event) => {
                    setBaseUnit(event.target.value as InventoryBaseUnit | '')
                    setPage(0)
                  }}
                >
                  <MenuItem value="">Todas</MenuItem>
                  {INVENTORY_BASE_UNITS.map((value) => (
                    <MenuItem key={value} value={value}>{BASE_UNIT_LABELS[value]}</MenuItem>
                  ))}
                </Select>
              </FormControl>
              <Button type="submit" variant="outlined">Buscar</Button>
            </Stack>
          )}
        </Stack>
      </Paper>

      {!branchId && (
        <Alert severity="info" role="alert">Selecciona una sucursal para ver sus existencias.</Alert>
      )}

      {stockQuery.isError && (
        <Alert severity="error" role="alert">{inventoryStockErrorMessage(stockQuery.error)}</Alert>
      )}

      {branchId && stockQuery.isPending && (
        <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
          <CircularProgress size={28} aria-label="Cargando existencias" />
        </Box>
      )}

      {branchId && stockQuery.isSuccess && (
        <TableContainer component={Paper} variant="outlined">
          <Table size="small" aria-label="Existencias de la sucursal">
            <TableHead>
              <TableRow>
                <TableCell>Código</TableCell>
                <TableCell>Nombre</TableCell>
                <TableCell>Unidad base</TableCell>
                <TableCell align="right">Existencia</TableCell>
                <TableCell>Estado</TableCell>
                <TableCell align="right">Acciones</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.inventoryItemId}>
                  <TableCell>{row.code}</TableCell>
                  <TableCell>{row.name}</TableCell>
                  <TableCell>{BASE_UNIT_LABELS[row.baseUnit]}</TableCell>
                  <TableCell align="right">{formatQuantity(row.quantityOnHand)}</TableCell>
                  <TableCell>
                    <Chip
                      size="small"
                      label={ITEM_STATUS_LABELS[row.status]}
                      color={row.status === 'ACTIVE' ? 'success' : 'default'}
                    />
                  </TableCell>
                  <TableCell align="right">
                    <Button
                      size="small"
                      component={Link}
                      to={`/admin/inventory/stock/${row.inventoryItemId}?branch=${encodeURIComponent(branchId)}`}
                    >
                      Ver
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
              {rows.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6}>
                    <Typography variant="body2" color="text.secondary">
                      No hay artículos de inventario para esta sucursal.
                    </Typography>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}

      {branchId && stockQuery.isSuccess && totalElements > 0 && (
        <Stack direction="row" spacing={2} sx={{ alignItems: 'center', justifyContent: 'flex-end' }}>
          <Typography variant="body2" color="text.secondary">
            Página {page + 1} de {Math.max(totalPages, 1)}
          </Typography>
          <Button size="small" disabled={page <= 0} onClick={() => setPage((value) => Math.max(value - 1, 0))}>
            Anterior
          </Button>
          <Button size="small" disabled={page + 1 >= totalPages} onClick={() => setPage((value) => value + 1)}>
            Siguiente
          </Button>
        </Stack>
      )}
    </Stack>
  )
}
