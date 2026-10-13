import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import CircularProgress from '@mui/material/CircularProgress'
import Paper from '@mui/material/Paper'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { QUANTITY_UNIT_SYMBOLS } from '../../inventory-items/api/inventoryItemsApi'
import { InventoryItemPicker } from '../../inventory-items/components/InventoryItemPicker'
import type { SelectedInventoryItem } from '../../inventory-items/components/InventoryItemPicker'
import { formatQuantity } from '../../inventory-stock/model/inventoryQuantity'
import { validateEffectDelta } from '../../inventory-recipes/model/inventoryRecipeValidation'
import {
  getModifierEffects,
  replaceModifierEffects,
} from '../api/inventoryModifierEffectsApi'
import type { ModifierEffects, ModifierOption } from '../api/inventoryModifierEffectsApi'
import { inventoryModifierEffectsKeys } from '../model/inventoryModifierEffectsKeys'
import { errorCode, inventoryModifierEffectsErrorMessage, isEffectsAbsent } from '../model/inventoryModifierEffectsErrors'

interface EditorRow {
  key: string
  item: SelectedInventoryItem | null
  quantityDelta: string
}

export interface ModifierEffectEditorProps {
  organizationId: string
  option: ModifierOption
  dirtyRef?: RefObject<boolean>
}

function effectItem(effect: ModifierEffects['effects'][number]): SelectedInventoryItem {
  return {
    id: effect.inventoryItemId,
    code: effect.code,
    name: effect.name,
    baseUnit: effect.baseUnit,
    fractionalUnitAllowed: effect.fractionalUnitAllowed,
    status: effect.status,
  }
}

export function ModifierEffectEditor({ organizationId, option, dirtyRef }: ModifierEffectEditorProps) {
  const [banner, setBanner] = useState('')
  const effectsQuery = useQuery({
    queryKey: inventoryModifierEffectsKeys.effects(organizationId, option.id),
    queryFn: () => getModifierEffects(organizationId, option.id),
    enabled: Boolean(organizationId),
    retry: false,
    refetchOnWindowFocus: false,
  })

  const absent = effectsQuery.isError && isEffectsAbsent(effectsQuery.error)
  const config = effectsQuery.data ?? null

  if (effectsQuery.isPending) {
    return (
      <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
        <CircularProgress size={28} aria-label="Cargando efectos" />
      </Box>
    )
  }

  if (effectsQuery.isError && !absent) {
    return <Alert severity="error" role="alert">{inventoryModifierEffectsErrorMessage(effectsQuery.error)}</Alert>
  }

  return (
    <Stack spacing={2}>
      {banner && (
        <Alert severity="info" role="status" onClose={() => setBanner('')}>{banner}</Alert>
      )}
      <ModifierEffectForm
        key={config ? `v:${config.version}` : 'absent'}
        organizationId={organizationId}
        option={option}
        config={config}
        dirtyRef={dirtyRef}
        onBanner={setBanner}
      />
    </Stack>
  )
}

interface ModifierEffectFormProps {
  organizationId: string
  option: ModifierOption
  config: ModifierEffects | null
  dirtyRef?: RefObject<boolean>
  onBanner: (message: string) => void
}

function ModifierEffectForm({ organizationId, option, config, dirtyRef, onBanner }: ModifierEffectFormProps) {
  const queryClient = useQueryClient()
  const keyRef = useRef(0)

  const [rows, setRows] = useState<EditorRow[]>(() =>
    config
      ? config.effects.map((effect) => ({
          key: effect.inventoryItemId,
          item: effectItem(effect),
          quantityDelta: formatQuantity(effect.quantityDelta),
        }))
      : [],
  )
  const [formError, setFormError] = useState('')

  const baseline = config
    ? JSON.stringify(config.effects.map((effect) => [effect.inventoryItemId, formatQuantity(effect.quantityDelta)]).sort())
    : JSON.stringify([])
  const current = JSON.stringify(rows.map((row) => [row.item?.id ?? '', row.quantityDelta.trim()]).sort())
  const dirty = baseline !== current

  useEffect(() => {
    if (dirtyRef) dirtyRef.current = dirty
  }, [dirty, dirtyRef])

  const saveMutation = useMutation({
    mutationFn: () =>
      replaceModifierEffects(organizationId, option.id, {
        expectedVersion: config?.version ?? 0,
        effects: rows.map((row) => ({
          inventoryItemId: row.item?.id ?? '',
          quantityDelta: row.quantityDelta.trim(),
        })),
      }),
    onSuccess: (saved) => {
      queryClient.setQueryData(inventoryModifierEffectsKeys.effects(organizationId, option.id), saved)
      onBanner('Efectos guardados.')
      void queryClient.invalidateQueries({
        queryKey: inventoryModifierEffectsKeys.effects(organizationId, option.id),
      })
    },
    onError: (error) => {
      onBanner(inventoryModifierEffectsErrorMessage(error))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION') {
        void queryClient.invalidateQueries({
          queryKey: inventoryModifierEffectsKeys.effects(organizationId, option.id),
        })
      }
    },
  })

  function addRow() {
    setRows((previous) => [...previous, { key: `new-${keyRef.current++}`, item: null, quantityDelta: '' }])
  }

  function removeRow(key: string) {
    setRows((previous) => previous.filter((row) => row.key !== key))
  }

  function setRowItem(key: string, item: SelectedInventoryItem | null) {
    setRows((previous) => previous.map((row) => (row.key === key ? { ...row, item } : row)))
  }

  function setRowDelta(key: string, quantityDelta: string) {
    setRows((previous) => previous.map((row) => (row.key === key ? { ...row, quantityDelta } : row)))
  }

  function handleSubmit() {
    for (const row of rows) {
      if (!row.item) {
        setFormError('Selecciona un artículo en cada efecto.')
        return
      }
      const message = validateEffectDelta(row.quantityDelta, row.item)
      if (message) {
        setFormError(message)
        return
      }
    }
    const ids = rows.map((row) => row.item?.id)
    if (new Set(ids).size !== ids.length) {
      setFormError('No puedes repetir el mismo artículo en los efectos.')
      return
    }
    setFormError('')
    saveMutation.mutate()
  }

  function handleCancel() {
    setRows(
      config
        ? config.effects.map((effect) => ({
            key: effect.inventoryItemId,
            item: effectItem(effect),
            quantityDelta: formatQuantity(effect.quantityDelta),
          }))
        : [],
    )
    setFormError('')
    onBanner('')
  }

  return (
    <Paper variant="outlined" sx={{ p: 2 }}>
      <Stack spacing={2}>
        <Box>
          <Typography component="h3" variant="h6">{option.name}</Typography>
          <Typography variant="body2" color="text.secondary">
            {option.code}
            {config ? '' : ' · Sin efectos configurados'}
          </Typography>
        </Box>

        {formError && <Alert severity="error" role="alert">{formError}</Alert>}

        <Alert severity="info">
          Un valor positivo agrega consumo a la receta base; un valor negativo lo resta. Ejemplo: −250 ml de leche
          normal y +250 ml de bebida de avena describen una sustitución como dos efectos.
        </Alert>

        <Stack spacing={2}>
          {rows.map((row) => {
            const otherIds = rows.filter((other) => other.key !== row.key && other.item).map((other) => other.item!.id)
            const deltaError = row.quantityDelta.trim() ? validateEffectDelta(row.quantityDelta, row.item) : null
            return (
              <Stack
                key={row.key}
                direction={{ xs: 'column', sm: 'row' }}
                spacing={2}
                sx={{ alignItems: { xs: 'stretch', sm: 'flex-start' } }}
              >
                <Box sx={{ flexGrow: 1, minWidth: 0 }}>
                  <InventoryItemPicker
                    organizationId={organizationId}
                    value={row.item}
                    onChange={(item) => setRowItem(row.key, item)}
                    disabledItemIds={otherIds}
                    error={!row.item && Boolean(formError)}
                  />
                </Box>
                <TextField
                  label="Cambio de consumo"
                  value={row.quantityDelta}
                  onChange={(event) => setRowDelta(row.key, event.target.value)}
                  error={Boolean(deltaError)}
                  helperText={deltaError ?? (row.item ? QUANTITY_UNIT_SYMBOLS[row.item.baseUnit] : ' ')}
                  sx={{ width: { xs: '100%', sm: 200 } }}
                />
                <Button
                  onClick={() => removeRow(row.key)}
                  aria-label={`Quitar efecto ${row.item ? row.item.name : ''}`.trim()}
                >
                  Quitar
                </Button>
              </Stack>
            )
          })}
          {rows.length === 0 && (
            <Typography variant="body2" color="text.secondary">
              Esta opción no tiene efectos de inventario configurados.
            </Typography>
          )}
        </Stack>

        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1}>
          <Button variant="outlined" onClick={addRow}>Añadir efecto</Button>
          <Box sx={{ flexGrow: 1 }} />
          <Button onClick={handleCancel} disabled={saveMutation.isPending}>Cancelar</Button>
          <Button variant="contained" onClick={handleSubmit} disabled={saveMutation.isPending}>
            Guardar efectos
          </Button>
        </Stack>
      </Stack>
    </Paper>
  )
}
