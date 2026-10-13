import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import CircularProgress from '@mui/material/CircularProgress'
import FormControl from '@mui/material/FormControl'
import FormControlLabel from '@mui/material/FormControlLabel'
import FormLabel from '@mui/material/FormLabel'
import Paper from '@mui/material/Paper'
import Radio from '@mui/material/Radio'
import RadioGroup from '@mui/material/RadioGroup'
import Stack from '@mui/material/Stack'
import Typography from '@mui/material/Typography'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useAuthSession } from '../../auth/model/useAuthSession'
import { getOrganization, updateOrganizationPolicy } from '../api/organizationApi'
import type { InventoryNegativeStockPolicy } from '../api/organizationApi'
import { organizationKeys } from '../model/organizationKeys'
import {
  INVENTORY_NEGATIVE_STOCK_POLICIES,
  INVENTORY_NEGATIVE_STOCK_POLICY_DESCRIPTIONS,
  INVENTORY_NEGATIVE_STOCK_POLICY_LABELS,
} from '../model/inventoryPolicy'
import { errorCode, inventorySettingsErrorMessage } from '../model/organizationSettingsErrors'

const LEGEND_ID = 'inventory-negative-stock-policy-legend'

export function InventorySettingsPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''
  const queryClient = useQueryClient()

  // Draft is the user's pending choice; null means "follow the persisted value".
  // Deriving the shown value from the query keeps the selection in sync on load,
  // after a save, and after a 409 refetch without setState-in-effect.
  const [draft, setDraft] = useState<InventoryNegativeStockPolicy | null>(null)
  const [successMessage, setSuccessMessage] = useState('')
  const [errorMessage, setErrorMessage] = useState('')

  const organizationQuery = useQuery({
    queryKey: organizationKeys.detail(organizationId),
    queryFn: () => getOrganization(organizationId),
    enabled: Boolean(organizationId),
  })

  const serverPolicy = organizationQuery.data?.inventoryNegativeStockPolicy ?? null
  const version = organizationQuery.data?.version
  const selected = draft ?? serverPolicy
  const dirty = draft !== null && draft !== serverPolicy

  const mutation = useMutation({
    mutationFn: (policy: InventoryNegativeStockPolicy) =>
      updateOrganizationPolicy(organizationId, {
        expectedVersion: version ?? 0,
        inventoryNegativeStockPolicy: policy,
      }),
    onSuccess: (updated) => {
      setErrorMessage('')
      setSuccessMessage('Configuración guardada.')
      setDraft(null)
      queryClient.setQueryData(organizationKeys.detail(organizationId), updated)
    },
    onError: (error) => {
      setSuccessMessage('')
      setErrorMessage(inventorySettingsErrorMessage(error))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION') {
        // Drop the stale draft and refetch so the latest server policy is shown.
        setDraft(null)
        void queryClient.invalidateQueries({ queryKey: organizationKeys.detail(organizationId) })
      }
    },
  })

  return (
    <Stack spacing={2}>
      <Box>
        <Typography component="h2" variant="h5">Configuración de inventario</Typography>
        <Typography variant="body2" color="text.secondary">
          Política a nivel de organización. Se aplica a todas las sucursales; no se configura por sucursal.
        </Typography>
      </Box>

      {successMessage && (
        <Alert severity="success" role="status" onClose={() => setSuccessMessage('')}>
          {successMessage}
        </Alert>
      )}
      {errorMessage && (
        <Alert severity="error" role="alert" onClose={() => setErrorMessage('')}>
          {errorMessage}
        </Alert>
      )}

      {organizationQuery.isPending && (
        <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
          <CircularProgress size={28} aria-label="Cargando configuración" />
        </Box>
      )}

      {organizationQuery.isError && (
        <Alert severity="error" role="alert">
          {inventorySettingsErrorMessage(organizationQuery.error)}
        </Alert>
      )}

      {organizationQuery.isSuccess && (
        <Paper variant="outlined" sx={{ p: 2 }}>
          <FormControl component="fieldset" disabled={mutation.isPending}>
            <FormLabel component="legend" id={LEGEND_ID}>Política de stock negativo</FormLabel>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, mb: 1.5 }}>
              Aplica a toda la organización. Solo afecta el consumo de inventario al completar una venta; no
              cambia los ajustes manuales ni el stock de apertura.
            </Typography>
            <RadioGroup
              aria-labelledby={LEGEND_ID}
              name="inventory-negative-stock-policy"
              value={selected ?? ''}
              onChange={(event) => setDraft(event.target.value as InventoryNegativeStockPolicy)}
            >
              {INVENTORY_NEGATIVE_STOCK_POLICIES.map((policy) => (
                <FormControlLabel
                  key={policy}
                  value={policy}
                  control={<Radio />}
                  label={
                    <Box>
                      <Stack direction="row" spacing={1} sx={{ alignItems: 'center', flexWrap: 'wrap' }}>
                        <Typography component="span">{INVENTORY_NEGATIVE_STOCK_POLICY_LABELS[policy]}</Typography>
                        <Chip size="small" variant="outlined" label={policy} />
                      </Stack>
                      <Typography variant="body2" color="text.secondary">
                        {INVENTORY_NEGATIVE_STOCK_POLICY_DESCRIPTIONS[policy]}
                      </Typography>
                    </Box>
                  }
                />
              ))}
            </RadioGroup>
          </FormControl>

          {selected === 'ALLOW_NEGATIVE' && (
            <Alert severity="warning" sx={{ mt: 1.5 }}>
              Las ventas podrán completarse aunque el stock rastreado sea insuficiente. El inventario puede quedar
              con cantidades negativas.
            </Alert>
          )}

          <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
            <Button
              variant="contained"
              disabled={mutation.isPending || selected === null || !dirty}
              onClick={() => {
                if (selected !== null) mutation.mutate(selected)
              }}
            >
              Guardar configuración
            </Button>
            {dirty && (
              <Button disabled={mutation.isPending} onClick={() => setDraft(null)}>
                Descartar cambios
              </Button>
            )}
          </Stack>
        </Paper>
      )}
    </Stack>
  )
}
