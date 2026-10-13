import Alert from '@mui/material/Alert'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogActions from '@mui/material/DialogActions'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import FormControl from '@mui/material/FormControl'
import FormControlLabel from '@mui/material/FormControlLabel'
import InputLabel from '@mui/material/InputLabel'
import MenuItem from '@mui/material/MenuItem'
import Select from '@mui/material/Select'
import Stack from '@mui/material/Stack'
import Switch from '@mui/material/Switch'
import TextField from '@mui/material/TextField'
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { createInventoryItem, updateInventoryItem } from '../api/inventoryItemsApi'
import type { InventoryBaseUnit, InventoryItem } from '../api/inventoryItemsApi'
import { BASE_UNIT_LABELS, INVENTORY_BASE_UNITS } from '../api/inventoryItemsApi'
import { errorCode, inventoryItemErrorMessage } from '../model/inventoryItemErrors'

const CODE_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/

type Props = {
  open: boolean
  organizationId: string
  item: InventoryItem | null
  onClose: () => void
  onSaved: () => void
}

export function InventoryItemFormDialog({ open, organizationId, item, onClose, onSaved }: Props) {
  const isEdit = Boolean(item)
  const [code, setCode] = useState(item?.code ?? '')
  const [name, setName] = useState(item?.name ?? '')
  const [description, setDescription] = useState(item?.description ?? '')
  const [baseUnit, setBaseUnit] = useState<InventoryBaseUnit>(item?.baseUnit ?? 'UNIT')
  const [fractionalUnitAllowed, setFractionalUnitAllowed] = useState(item?.fractionalUnitAllowed ?? false)
  const [submitError, setSubmitError] = useState('')

  const mutation = useMutation({
    mutationFn: () => {
      const trimmedName = name.trim()
      const trimmedDescription = description.trim()
      if (item) {
        return updateInventoryItem(organizationId, item.id, {
          expectedVersion: item.version,
          name: trimmedName,
          description: trimmedDescription,
          baseUnit,
          fractionalUnitAllowed,
        })
      }
      return createInventoryItem(organizationId, {
        code: code.trim(),
        name: trimmedName,
        description: trimmedDescription || undefined,
        baseUnit,
        fractionalUnitAllowed,
      })
    },
    onSuccess: () => {
      onSaved()
      onClose()
    },
    onError: (error) => {
      setSubmitError(inventoryItemErrorMessage(error))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION') onSaved()
    },
  })

  function validate(): string | null {
    const trimmedCode = code.trim()
    const trimmedName = name.trim()
    if (!isEdit) {
      if (!trimmedCode) return 'Ingresa un código.'
      if (trimmedCode.length > 64) return 'El código admite hasta 64 caracteres.'
      if (!CODE_PATTERN.test(trimmedCode)) return 'El código debe ser un slug en minúsculas (ej. cafe-molido).'
    }
    if (!trimmedName) return 'Ingresa un nombre.'
    if (trimmedName.length > 120) return 'El nombre admite hasta 120 caracteres.'
    if (description.trim().length > 1000) return 'La descripción admite hasta 1000 caracteres.'
    return null
  }

  function handleSubmit() {
    const validation = validate()
    if (validation) {
      setSubmitError(validation)
      return
    }
    setSubmitError('')
    mutation.mutate()
  }

  return (
    <Dialog open={open} onClose={mutation.isPending ? undefined : onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? `Editar ${item?.name ?? 'artículo'}` : 'Nuevo artículo de inventario'}</DialogTitle>
      <DialogContent>
        <Stack
          component="form"
          spacing={2}
          sx={{ pt: 1 }}
          onSubmit={(event) => {
            event.preventDefault()
            handleSubmit()
          }}
        >
          {submitError && <Alert severity="error" role="alert">{submitError}</Alert>}
          {isEdit ? (
            <TextField
              label="Código"
              value={item?.code ?? ''}
              disabled
              helperText="El código es inmutable."
            />
          ) : (
            <TextField
              label="Código"
              value={code}
              onChange={(event) => setCode(event.target.value)}
              required
              helperText="Slug en minúsculas, ej. cafe-molido."
              slotProps={{ htmlInput: { maxLength: 64 } }}
            />
          )}
          <TextField
            label="Nombre"
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            slotProps={{ htmlInput: { maxLength: 120 } }}
          />
          <TextField
            label="Descripción"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            multiline
            minRows={2}
            slotProps={{ htmlInput: { maxLength: 1000 } }}
          />
          <FormControl fullWidth>
            <InputLabel id="base-unit-label">Unidad base</InputLabel>
            <Select
              labelId="base-unit-label"
              label="Unidad base"
              value={baseUnit}
              onChange={(event) => setBaseUnit(event.target.value as InventoryBaseUnit)}
            >
              {INVENTORY_BASE_UNITS.map((unit) => (
                <MenuItem key={unit} value={unit}>{BASE_UNIT_LABELS[unit]}</MenuItem>
              ))}
            </Select>
          </FormControl>
          <FormControlLabel
            control={
              <Switch
                checked={fractionalUnitAllowed}
                onChange={(event) => setFractionalUnitAllowed(event.target.checked)}
              />
            }
            label="Permite cantidades fraccionarias"
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose} disabled={mutation.isPending}>Cancelar</Button>
        <Button variant="contained" onClick={handleSubmit} disabled={mutation.isPending}>
          {isEdit ? 'Guardar cambios' : 'Crear artículo'}
        </Button>
      </DialogActions>
    </Dialog>
  )
}
