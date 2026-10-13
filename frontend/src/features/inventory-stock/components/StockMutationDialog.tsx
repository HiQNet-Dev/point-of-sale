import Alert from '@mui/material/Alert'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogActions from '@mui/material/DialogActions'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { BASE_UNIT_LABELS } from '../../inventory-items/api/inventoryItemsApi'
import type { BranchStock } from '../api/inventoryStockApi'
import { recordOpeningStock, recordStockAdjustment } from '../api/inventoryStockApi'
import { errorCode, inventoryStockErrorMessage } from '../model/inventoryStockErrors'
import { formatQuantity, formatScaledQuantity, parseScaledQuantity, validateAdjustmentDelta, validateOpeningQuantity } from '../model/inventoryQuantity'

export type StockMutationMode = 'opening' | 'adjustment'

type Props = {
  open: boolean
  mode: StockMutationMode
  organizationId: string
  branchId: string
  stock: BranchStock | null
  onClose: () => void
  onSaved: () => void
}

export function StockMutationDialog({ open, mode, organizationId, branchId, stock, onClose, onSaved }: Props) {
  const isOpening = mode === 'opening'
  const [quantity, setQuantity] = useState('')
  const [reason, setReason] = useState('')
  const [submitError, setSubmitError] = useState('')

  const mutation = useMutation({
    mutationFn: () => {
      if (!stock) throw new Error('No stock selected')
      if (isOpening) {
        return recordOpeningStock(organizationId, branchId, stock.inventoryItemId, {
          expectedVersion: stock.version,
          quantity: quantity.trim(),
          reason: reason.trim() || undefined,
        })
      }
      return recordStockAdjustment(organizationId, branchId, stock.inventoryItemId, {
        expectedVersion: stock.version,
        quantityDelta: quantity.trim(),
        reason: reason.trim(),
      })
    },
    onSuccess: () => {
      onSaved()
      onClose()
    },
    onError: (error) => {
      setSubmitError(inventoryStockErrorMessage(error))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION') onSaved()
    },
  })

  function validate(): string | null {
    const validation = isOpening ? validateOpeningQuantity(quantity) : validateAdjustmentDelta(quantity)
    if (validation) return validation
    const trimmedReason = reason.trim()
    if (!isOpening && !trimmedReason) return 'Ingresa un motivo para el ajuste.'
    if (trimmedReason.length > 255) return 'El motivo admite hasta 255 caracteres.'
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

  const currentScaled = stock ? parseScaledQuantity(stock.quantityOnHand) : null
  const deltaScaled = parseScaledQuantity(quantity)
  const projectedScaled = currentScaled !== null && deltaScaled !== null ? currentScaled + deltaScaled : null

  return (
    <Dialog open={open} onClose={mutation.isPending ? undefined : onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isOpening ? 'Apertura de existencias' : 'Ajuste de existencias'}</DialogTitle>
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
          {stock && (
            <Typography variant="body2" color="text.secondary">
              {stock.code} · {stock.name} — existencia actual {formatQuantity(stock.quantityOnHand)}{' '}
              {BASE_UNIT_LABELS[stock.baseUnit]}
            </Typography>
          )}
          <TextField
            label={isOpening ? 'Cantidad de apertura' : 'Ajuste (+/-)'}
            value={quantity}
            onChange={(event) => setQuantity(event.target.value)}
            required
            autoFocus
            helperText={isOpening ? 'Cero o mayor, hasta 6 decimales.' : 'Puede ser negativo; no puede ser cero.'}
            slotProps={{ htmlInput: { inputMode: 'decimal' } }}
          />
          {!isOpening && projectedScaled !== null && (
            <Typography variant="body2" color="text.secondary">
              Existencia resultante estimada: {formatScaledQuantity(projectedScaled)}
            </Typography>
          )}
          <TextField
            label={isOpening ? 'Motivo (opcional)' : 'Motivo'}
            value={reason}
            onChange={(event) => setReason(event.target.value)}
            required={!isOpening}
            multiline
            minRows={2}
            helperText="Hasta 255 caracteres."
            slotProps={{ htmlInput: { maxLength: 255 } }}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose} disabled={mutation.isPending}>Cancelar</Button>
        <Button variant="contained" onClick={handleSubmit} disabled={mutation.isPending || !stock}>
          {isOpening ? 'Registrar apertura' : 'Registrar ajuste'}
        </Button>
      </DialogActions>
    </Dialog>
  )
}
