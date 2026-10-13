import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import CircularProgress from '@mui/material/CircularProgress'
import Paper from '@mui/material/Paper'
import Stack from '@mui/material/Stack'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Typography from '@mui/material/Typography'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { Link, useParams, useSearchParams } from 'react-router-dom'
import { useAuthSession } from '../../auth/model/useAuthSession'
import { BASE_UNIT_LABELS, ITEM_STATUS_LABELS } from '../../inventory-items/api/inventoryItemsApi'
import { getBranchStock, listInventoryMovements, MOVEMENT_TYPE_LABELS } from '../api/inventoryStockApi'
import { inventoryStockKeys } from '../model/inventoryStockKeys'
import { inventoryStockErrorMessage } from '../model/inventoryStockErrors'
import { formatQuantity } from '../model/inventoryQuantity'
import { StockMutationDialog } from '../components/StockMutationDialog'
import type { StockMutationMode } from '../components/StockMutationDialog'

const MOVEMENTS_PAGE_SIZE = 20

export function BranchStockDetailPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''
  const locale = me?.organization.locale ?? 'es-MX'
  const timezone = me?.organization.timezone ?? 'UTC'
  const params = useParams()
  const inventoryItemId = params.itemId ?? ''
  const [searchParams] = useSearchParams()
  const branchId = searchParams.get('branch') ?? ''
  const queryClient = useQueryClient()

  const [mutationMode, setMutationMode] = useState<StockMutationMode | null>(null)
  const [movementsPage, setMovementsPage] = useState(0)

  const ready = Boolean(organizationId) && Boolean(branchId) && Boolean(inventoryItemId)

  const stockQuery = useQuery({
    queryKey: inventoryStockKeys.detail(organizationId, branchId, inventoryItemId),
    queryFn: () => getBranchStock(organizationId, branchId, inventoryItemId),
    enabled: ready,
  })

  const movementsQuery = useQuery({
    queryKey: inventoryStockKeys.movements(organizationId, branchId, inventoryItemId, movementsPage),
    queryFn: () => listInventoryMovements(organizationId, branchId, inventoryItemId, movementsPage, MOVEMENTS_PAGE_SIZE),
    enabled: ready,
    placeholderData: (previous) => previous,
  })

  function refresh() {
    void queryClient.invalidateQueries({ queryKey: inventoryStockKeys.all })
  }

  function formatOccurredAt(value: string): string {
    return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short', timeZone: timezone })
      .format(new Date(value))
  }

  if (!ready) {
    return <Alert severity="error" role="alert">Falta la sucursal o el artículo seleccionado.</Alert>
  }

  const stock = stockQuery.data ?? null
  const movements = movementsQuery.data?.items ?? []
  const movementsTotal = movementsQuery.data?.totalElements ?? 0
  const movementsTotalPages = movementsQuery.data?.totalPages ?? 0
  const openingAllowed = stock !== null && stock.version === 0

  return (
    <Stack spacing={2}>
      <Box>
        <Button component={Link} to={`/admin/inventory/stock?branch=${encodeURIComponent(branchId)}`} size="small">
          Volver a existencias
        </Button>
      </Box>

      {stockQuery.isError && (
        <Alert severity="error" role="alert">{inventoryStockErrorMessage(stockQuery.error)}</Alert>
      )}

      {stockQuery.isPending && (
        <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
          <CircularProgress size={28} aria-label="Cargando existencias" />
        </Box>
      )}

      {stock && (
        <>
          <Paper variant="outlined" sx={{ p: 2 }}>
            <Stack spacing={1.5}>
              <Stack direction="row" spacing={1} sx={{ alignItems: 'center', flexWrap: 'wrap' }}>
                <Typography component="h2" variant="h5">{stock.name}</Typography>
                <Chip size="small" label={ITEM_STATUS_LABELS[stock.status]} color={stock.status === 'ACTIVE' ? 'success' : 'default'} />
              </Stack>
              <Box component="dl" sx={{ display: 'grid', gridTemplateColumns: 'minmax(140px, 1fr) 2fr', gap: 1, m: 0 }}>
                <Typography component="dt" variant="body2" color="text.secondary">Código</Typography>
                <Typography component="dd" variant="body2" sx={{ m: 0 }}>{stock.code}</Typography>
                <Typography component="dt" variant="body2" color="text.secondary">Unidad base</Typography>
                <Typography component="dd" variant="body2" sx={{ m: 0 }}>
                  {BASE_UNIT_LABELS[stock.baseUnit]}{stock.fractionalUnitAllowed ? ' (admite fracciones)' : ''}
                </Typography>
                <Typography component="dt" variant="body2" color="text.secondary">Existencia</Typography>
                <Typography component="dd" variant="body2" sx={{ m: 0, fontWeight: 700 }}>
                  {formatQuantity(stock.quantityOnHand)} {BASE_UNIT_LABELS[stock.baseUnit]}
                </Typography>
                <Typography component="dt" variant="body2" color="text.secondary">Versión</Typography>
                <Typography component="dd" variant="body2" sx={{ m: 0 }}>{stock.version}</Typography>
              </Box>
              <Stack direction="row" spacing={1}>
                <Button variant="contained" disabled={!openingAllowed} onClick={() => setMutationMode('opening')}>
                  Apertura de existencias
                </Button>
                <Button variant="outlined" onClick={() => setMutationMode('adjustment')}>
                  Ajuste de existencias
                </Button>
              </Stack>
              {!openingAllowed && (
                <Typography variant="body2" color="text.secondary">
                  La apertura solo se permite antes de cualquier movimiento de existencias.
                </Typography>
              )}
            </Stack>
          </Paper>

          <Box>
            <Typography component="h3" variant="h6" sx={{ mb: 1 }}>Historial de movimientos</Typography>
            {movementsQuery.isError && (
              <Alert severity="error" role="alert">{inventoryStockErrorMessage(movementsQuery.error)}</Alert>
            )}
            {movementsQuery.isSuccess && (
              <>
                <TableContainer component={Paper} variant="outlined">
                  <Table size="small" aria-label="Historial de movimientos">
                    <TableHead>
                      <TableRow>
                        <TableCell>Fecha</TableCell>
                        <TableCell>Tipo</TableCell>
                        <TableCell align="right">Cambio</TableCell>
                        <TableCell align="right">Resultante</TableCell>
                        <TableCell>Motivo</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {movements.map((movement) => (
                        <TableRow key={movement.id}>
                          <TableCell>{formatOccurredAt(movement.occurredAt)}</TableCell>
                          <TableCell>{MOVEMENT_TYPE_LABELS[movement.type]}</TableCell>
                          <TableCell align="right">{formatQuantity(movement.quantityDelta)}</TableCell>
                          <TableCell align="right">{formatQuantity(movement.resultingQuantity)}</TableCell>
                          <TableCell>{movement.reason ?? '—'}</TableCell>
                        </TableRow>
                      ))}
                      {movements.length === 0 && (
                        <TableRow>
                          <TableCell colSpan={5}>
                            <Typography variant="body2" color="text.secondary">
                              Aún no hay movimientos para este artículo en esta sucursal.
                            </Typography>
                          </TableCell>
                        </TableRow>
                      )}
                    </TableBody>
                  </Table>
                </TableContainer>
                {movementsTotal > 0 && (
                  <Stack direction="row" spacing={2} sx={{ alignItems: 'center', justifyContent: 'flex-end', mt: 1 }}>
                    <Typography variant="body2" color="text.secondary">
                      Página {movementsPage + 1} de {Math.max(movementsTotalPages, 1)}
                    </Typography>
                    <Button size="small" disabled={movementsPage <= 0} onClick={() => setMovementsPage((value) => Math.max(value - 1, 0))}>
                      Anterior
                    </Button>
                    <Button size="small" disabled={movementsPage + 1 >= movementsTotalPages} onClick={() => setMovementsPage((value) => value + 1)}>
                      Siguiente
                    </Button>
                  </Stack>
                )}
              </>
            )}
          </Box>
        </>
      )}

      {stock && mutationMode !== null && (
        <StockMutationDialog
          key={`${mutationMode}-${stock.inventoryItemId}-${stock.version}`}
          open
          mode={mutationMode}
          organizationId={organizationId}
          branchId={branchId}
          stock={stock}
          onClose={() => setMutationMode(null)}
          onSaved={refresh}
        />
      )}
    </Stack>
  )
}
