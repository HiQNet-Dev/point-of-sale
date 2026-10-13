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
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useAuthSession } from '../../auth/model/useAuthSession'
import {
  BASE_UNIT_LABELS,
  INVENTORY_BASE_UNITS,
  INVENTORY_ITEMS_PAGE_SIZE,
  INVENTORY_ITEM_STATUSES,
  ITEM_STATUS_LABELS,
  listInventoryItems,
  updateInventoryItem,
} from '../api/inventoryItemsApi'
import type { InventoryBaseUnit, InventoryItem, InventoryItemListQuery, InventoryItemStatus } from '../api/inventoryItemsApi'
import { inventoryItemsKeys } from '../model/inventoryItemsKeys'
import { inventoryItemErrorMessage } from '../model/inventoryItemErrors'
import { InventoryItemFormDialog } from '../components/InventoryItemFormDialog'

export function InventoryItemsPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''
  const queryClient = useQueryClient()

  const [qInput, setQInput] = useState('')
  const [q, setQ] = useState('')
  const [status, setStatus] = useState<InventoryItemStatus | ''>('')
  const [baseUnit, setBaseUnit] = useState<InventoryBaseUnit | ''>('')
  const [page, setPage] = useState(0)
  const [creating, setCreating] = useState(false)
  const [editing, setEditing] = useState<InventoryItem | null>(null)
  const [banner, setBanner] = useState('')

  const query: InventoryItemListQuery = {
    page,
    size: INVENTORY_ITEMS_PAGE_SIZE,
    q,
    status: status || undefined,
    baseUnit: baseUnit || undefined,
  }

  const listQuery = useQuery({
    queryKey: inventoryItemsKeys.list(organizationId, query),
    queryFn: () => listInventoryItems(organizationId, query),
    enabled: Boolean(organizationId),
    placeholderData: (previous) => previous,
  })

  const statusMutation = useMutation({
    mutationFn: ({ item, next }: { item: InventoryItem; next: InventoryItemStatus }) =>
      updateInventoryItem(organizationId, item.id, { expectedVersion: item.version, status: next }),
    onSuccess: () => {
      setBanner('')
      void queryClient.invalidateQueries({ queryKey: inventoryItemsKeys.all })
    },
    onError: (error) => setBanner(inventoryItemErrorMessage(error)),
  })

  function invalidate() {
    void queryClient.invalidateQueries({ queryKey: inventoryItemsKeys.all })
  }

  const items = listQuery.data?.items ?? []
  const totalPages = listQuery.data?.totalPages ?? 0
  const totalElements = listQuery.data?.totalElements ?? 0

  return (
    <Stack spacing={2}>
      <Box>
        <Typography component="h2" variant="h5">Artículos de inventario</Typography>
        <Typography variant="body2" color="text.secondary">
          Catálogo de artículos de la organización. Las existencias por sucursal se gestionan por separado.
        </Typography>
      </Box>

      {banner && <Alert severity="error" role="alert" onClose={() => setBanner('')}>{banner}</Alert>}

      <Paper variant="outlined" sx={{ p: 2 }}>
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
            <InputLabel id="item-status-label">Estado</InputLabel>
            <Select
              labelId="item-status-label"
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
            <InputLabel id="item-unit-label">Unidad base</InputLabel>
            <Select
              labelId="item-unit-label"
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
          <Stack direction="row" spacing={1}>
            <Button type="submit" variant="outlined">Buscar</Button>
            <Button variant="contained" onClick={() => setCreating(true)}>Nuevo artículo</Button>
          </Stack>
        </Stack>
      </Paper>

      {listQuery.isError && (
        <Alert severity="error" role="alert">{inventoryItemErrorMessage(listQuery.error)}</Alert>
      )}

      {listQuery.isPending && (
        <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
          <CircularProgress size={28} aria-label="Cargando artículos" />
        </Box>
      )}

      {listQuery.isSuccess && (
        <TableContainer component={Paper} variant="outlined">
          <Table size="small" aria-label="Artículos de inventario">
            <TableHead>
              <TableRow>
                <TableCell>Código</TableCell>
                <TableCell>Nombre</TableCell>
                <TableCell>Unidad base</TableCell>
                <TableCell>Fraccionable</TableCell>
                <TableCell>Estado</TableCell>
                <TableCell align="right">Acciones</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {items.map((item) => (
                <TableRow key={item.id}>
                  <TableCell>{item.code}</TableCell>
                  <TableCell>{item.name}</TableCell>
                  <TableCell>{BASE_UNIT_LABELS[item.baseUnit]}</TableCell>
                  <TableCell>{item.fractionalUnitAllowed ? 'Sí' : 'No'}</TableCell>
                  <TableCell>
                    <Chip
                      size="small"
                      label={ITEM_STATUS_LABELS[item.status]}
                      color={item.status === 'ACTIVE' ? 'success' : 'default'}
                    />
                  </TableCell>
                  <TableCell align="right">
                    <Stack direction="row" spacing={1} sx={{ justifyContent: 'flex-end' }}>
                      <Button size="small" onClick={() => setEditing(item)}>Editar</Button>
                      <Button
                        size="small"
                        disabled={statusMutation.isPending}
                        onClick={() => statusMutation.mutate({
                          item,
                          next: item.status === 'ACTIVE' ? 'INACTIVE' : 'ACTIVE',
                        })}
                      >
                        {item.status === 'ACTIVE' ? 'Desactivar' : 'Activar'}
                      </Button>
                    </Stack>
                  </TableCell>
                </TableRow>
              ))}
              {items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6}>
                    <Typography variant="body2" color="text.secondary">No hay artículos de inventario.</Typography>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}

      {listQuery.isSuccess && totalElements > 0 && (
        <Stack direction="row" spacing={2} sx={{ alignItems: 'center', justifyContent: 'flex-end' }}>
          <Typography variant="body2" color="text.secondary">
            Página {page + 1} de {Math.max(totalPages, 1)}
          </Typography>
          <Button size="small" disabled={page <= 0} onClick={() => setPage((value) => Math.max(value - 1, 0))}>
            Anterior
          </Button>
          <Button
            size="small"
            disabled={page + 1 >= totalPages}
            onClick={() => setPage((value) => value + 1)}
          >
            Siguiente
          </Button>
        </Stack>
      )}

      {(creating || editing !== null) && (
        <InventoryItemFormDialog
          key={editing?.id ?? 'new'}
          open
          organizationId={organizationId}
          item={editing}
          onClose={() => {
            setCreating(false)
            setEditing(null)
          }}
          onSaved={invalidate}
        />
      )}
    </Stack>
  )
}
