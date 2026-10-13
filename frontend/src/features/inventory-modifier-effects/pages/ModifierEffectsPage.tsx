import Alert from '@mui/material/Alert'
import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Paper from '@mui/material/Paper'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useAuthSession } from '../../auth/model/useAuthSession'
import {
  listModifierGroups,
  listModifierOptions,
  MODIFIER_GROUPS_PAGE_SIZE,
  MODIFIER_OPTIONS_PAGE_SIZE,
  MODIFIER_STATUS_LABELS,
} from '../api/inventoryModifierEffectsApi'
import type { ModifierGroup, ModifierOption } from '../api/inventoryModifierEffectsApi'
import { inventoryModifierEffectsKeys } from '../model/inventoryModifierEffectsKeys'
import { inventoryModifierEffectsErrorMessage } from '../model/inventoryModifierEffectsErrors'
import { ModifierEffectEditor } from '../components/ModifierEffectEditor'

export function ModifierEffectsPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''

  const [groupInput, setGroupInput] = useState('')
  const [group, setGroup] = useState<ModifierGroup | null>(null)
  const [option, setOption] = useState<ModifierOption | null>(null)
  const dirtyRef = useRef(false)

  const groupsQuery = useQuery({
    queryKey: inventoryModifierEffectsKeys.groups(organizationId, { page: 0, size: MODIFIER_GROUPS_PAGE_SIZE, q: groupInput }),
    queryFn: () => listModifierGroups(organizationId, { page: 0, size: MODIFIER_GROUPS_PAGE_SIZE, q: groupInput }),
    enabled: Boolean(organizationId),
  })

  const optionsQuery = useQuery({
    queryKey: inventoryModifierEffectsKeys.options(organizationId, group?.id ?? ''),
    queryFn: () => listModifierOptions(organizationId, group!.id, { page: 0, size: MODIFIER_OPTIONS_PAGE_SIZE }),
    enabled: Boolean(organizationId) && Boolean(group),
  })

  function discardGuard(): boolean {
    if (!dirtyRef.current) return true
    return window.confirm('Tienes cambios sin guardar en los efectos. ¿Deseas descartarlos?')
  }

  function selectGroup(next: ModifierGroup | null) {
    if (next?.id === group?.id) return
    if (!discardGuard()) return
    dirtyRef.current = false
    setGroup(next)
    setOption(null)
  }

  function selectOption(next: ModifierOption | null) {
    if (next?.id === option?.id) return
    if (!discardGuard()) return
    dirtyRef.current = false
    setOption(next)
  }

  return (
    <Stack spacing={2}>
      <Box>
        <Typography component="h2" variant="h5">Efectos de modificadores</Typography>
        <Typography variant="body2" color="text.secondary">
          Un efecto modifica el consumo de la receta base cuando se elige una opción. Pertenece a la organización y no
          depende de la sucursal.
        </Typography>
      </Box>

      <Paper variant="outlined" sx={{ p: 2 }}>
        <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
          <Autocomplete
            sx={{ flexGrow: 1, minWidth: 0 }}
            options={groupsQuery.data?.items ?? []}
            value={group}
            inputValue={groupInput}
            onInputChange={(_event, next) => setGroupInput(next)}
            onChange={(_event, next) => selectGroup(next)}
            getOptionLabel={(candidate) => `${candidate.name} · ${candidate.code}`}
            isOptionEqualToValue={(candidate, value) => candidate.id === value.id}
            loading={groupsQuery.isPending}
            loadingText="Cargando grupos…"
            noOptionsText="Sin grupos"
            openOnFocus
            renderOption={(props, candidate) => {
              const { key, ...rest } = props as { key: string }
              return (
                <Box component="li" key={key} {...rest} sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                  <Typography variant="body2" sx={{ flexGrow: 1 }}>{candidate.name} · {candidate.code}</Typography>
                  <Chip
                    size="small"
                    label={MODIFIER_STATUS_LABELS[candidate.status]}
                    color={candidate.status === 'ACTIVE' ? 'success' : 'default'}
                  />
                </Box>
              )
            }}
            renderInput={(params) => <TextField {...params} label="Grupo de modificadores" />}
          />

          <Autocomplete
            sx={{ flexGrow: 1, minWidth: 0 }}
            options={optionsQuery.data?.items ?? []}
            value={option}
            onChange={(_event, next) => selectOption(next)}
            getOptionLabel={(candidate) => `${candidate.name} · ${candidate.code}`}
            isOptionEqualToValue={(candidate, value) => candidate.id === value.id}
            disabled={!group}
            loading={optionsQuery.isPending}
            loadingText="Cargando opciones…"
            noOptionsText={group ? 'Sin opciones' : 'Selecciona un grupo'}
            openOnFocus
            renderOption={(props, candidate) => {
              const { key, ...rest } = props as { key: string }
              return (
                <Box component="li" key={key} {...rest} sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                  <Typography variant="body2" sx={{ flexGrow: 1 }}>{candidate.name} · {candidate.code}</Typography>
                  <Chip
                    size="small"
                    label={MODIFIER_STATUS_LABELS[candidate.status]}
                    color={candidate.status === 'ACTIVE' ? 'success' : 'default'}
                  />
                </Box>
              )
            }}
            renderInput={(params) => <TextField {...params} label="Opción de modificador" />}
          />
        </Stack>
        {groupsQuery.isError && (
          <Box sx={{ mt: 2 }}>
            <Alert severity="error" role="alert">{inventoryModifierEffectsErrorMessage(groupsQuery.error)}</Alert>
          </Box>
        )}
        {optionsQuery.isError && (
          <Box sx={{ mt: 2 }}>
            <Alert severity="error" role="alert">{inventoryModifierEffectsErrorMessage(optionsQuery.error)}</Alert>
          </Box>
        )}
      </Paper>

      {option ? (
        <Box>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            Grupo: {group?.name ?? '—'}
          </Typography>
          <ModifierEffectEditor
            key={option.id}
            organizationId={organizationId}
            option={option}
            dirtyRef={dirtyRef}
          />
        </Box>
      ) : (
        <Alert severity="info" role="status">
          Selecciona un grupo y una opción para configurar sus efectos de inventario.
        </Alert>
      )}
    </Stack>
  )
}
