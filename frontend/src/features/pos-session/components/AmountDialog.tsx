import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogActions from '@mui/material/DialogActions'
import DialogContent from '@mui/material/DialogContent'
import DialogContentText from '@mui/material/DialogContentText'
import DialogTitle from '@mui/material/DialogTitle'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import { useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { validateAmount } from '../model/moneyInput'

interface AmountDialogProps {
  open: boolean
  mode: 'open' | 'close'
  pending: boolean
  onClose(): void
  onConfirm(amount: string): Promise<void>
}

export function AmountDialog({ open, mode, pending, onClose, onConfirm }: AmountDialogProps) {
  const [amount, setAmount] = useState('')
  const [validationError, setValidationError] = useState<string | null>(null)
  const submitting = useRef(false)
  const isOpen = mode === 'open'
  const title = isOpen ? 'Abrir turno' : 'Cerrar turno'
  const fieldLabel = isOpen ? 'Fondo inicial' : 'Efectivo declarado'
  const submitLabel = isOpen ? 'Abrir turno' : 'Cerrar turno'

  function close() {
    if (pending || submitting.current) return
    setAmount('')
    setValidationError(null)
    onClose()
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (pending || submitting.current) return
    const error = validateAmount(amount)
    if (error) {
      setValidationError(error)
      return
    }

    submitting.current = true
    setValidationError(null)
    try {
      await onConfirm(amount)
      setAmount('')
    } catch {
      // The parent displays a safe domain or network message.
    } finally {
      submitting.current = false
    }
  }

  return (
    <Dialog open={open} onClose={close} fullWidth maxWidth="xs" aria-labelledby="shift-amount-title">
      <form onSubmit={submit} noValidate>
        <DialogTitle id="shift-amount-title">{title}</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <DialogContentText>
              {isOpen ? 'Indica el efectivo disponible al iniciar el turno.' : 'Ingresa el efectivo contado al finalizar el turno.'}
            </DialogContentText>
            <TextField
              autoFocus
              fullWidth
              id="shift-amount"
              label={fieldLabel}
              value={amount}
              onChange={(event) => {
                setAmount(event.target.value)
                if (validationError) setValidationError(null)
              }}
              error={Boolean(validationError)}
              helperText={validationError ?? 'Monto no negativo; hasta 4 decimales.'}
              slotProps={{
                htmlInput: { inputMode: 'decimal', 'aria-describedby': 'shift-amount-help' },
                formHelperText: { id: 'shift-amount-help' },
              }}
              disabled={pending}
            />
          </Stack>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2.5 }}>
          <Button onClick={close} disabled={pending}>Cancelar</Button>
          <Button type="submit" variant="contained" disabled={pending}>
            {pending ? 'Procesando…' : submitLabel}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  )
}
