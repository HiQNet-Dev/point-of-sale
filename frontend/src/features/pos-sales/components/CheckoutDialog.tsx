import { Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, FormControl, FormHelperText, InputLabel, MenuItem, Select, Stack, TextField, Typography } from '@mui/material'
import { useMutation } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'
import { checkoutSale } from '../api/posSalesApi'
import type { PaymentMethod, SaleDetail, SaleSummary } from '../api/posSalesApi'
import {
  addExactMoney,
  compareExactMoney,
  exactMoneyToDecimal,
  formatExactMoneyUnits,
  parseExactMoney,
  subtractExactMoney,
} from '../model/checkoutMoney'

type PaymentDraft = {
  id: number
  method: PaymentMethod | ''
  amount: string
  tenderedAmount: string
}

type Props = {
  organizationId: string
  branchId: string
  registerId: string
  shiftId: string
  sale: SaleSummary
  activeItemCount: number
  operationalParentsActive: boolean
  locale: string
  currency: string
  onClose: () => void
  onCompleted: (detail: SaleDetail) => void
  onReconcile: (saleId: string, message: string, refreshParents: boolean, terminal: boolean) => void
}

function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

function checkoutErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION': return 'La venta cambió. Revisa el total actualizado antes de volver a cobrar.'
    case 'PAYMENT_TOTAL_MISMATCH': return 'El importe de la venta cambió o los pagos no cubrieron el total. Revisa la venta y vuelve a iniciar el cobro.'
    case 'SALE_ALREADY_COMPLETED': return 'La venta ya fue completada.'
    case 'SALE_ALREADY_CANCELLED':
    case 'SALE_NOT_OPEN': return 'La venta ya no está abierta.'
    case 'SALE_HAS_NO_ACTIVE_ITEMS': return 'La venta ya no tiene productos activos.'
    case 'INSUFFICIENT_INVENTORY': return 'No hay inventario suficiente para completar la venta. Ajusta las existencias o modifica los productos de la venta.'
    case 'BRANCH_INACTIVE': return 'La sucursal está inactiva. Se actualizará el contexto operacional.'
    case 'CASH_REGISTER_INACTIVE': return 'La caja está inactiva. Se actualizará el contexto operacional.'
    case 'CASH_SHIFT_NOT_OPEN': return 'El turno ya no está abierto. Se actualizará el contexto operacional.'
    case 'INVALID_PAYMENT': return 'El backend rechazó uno de los importes. Revisa los datos del pago.'
    case 'INVALID_CASH_TENDERED_AMOUNT': return 'El efectivo recibido debe cubrir el importe aplicado.'
    case 'TOO_MANY_PAYMENTS': return 'Se permiten hasta 10 métodos de pago.'
    case 'MULTIPLE_CASH_PAYMENTS_NOT_ALLOWED': return 'Solo se permite un pago en efectivo por venta.'
    default:
      if (error instanceof ApiRequestError && error.status === 403) return 'No tienes permiso para completar esta venta.'
      if (error instanceof ApiNetworkError) return 'No se confirmó la respuesta del servidor. Se actualizará la venta antes de continuar.'
      return 'No se pudo confirmar el cobro. Se actualizará la venta antes de continuar.'
  }
}

function isRecoverableCheckoutError(error: unknown): boolean {
  return !['INVALID_PAYMENT', 'INVALID_CASH_TENDERED_AMOUNT', 'TOO_MANY_PAYMENTS', 'MULTIPLE_CASH_PAYMENTS_NOT_ALLOWED', 'INSUFFICIENT_INVENTORY']
    .includes(errorCode(error) ?? '')
}

function paymentRow(id: number): PaymentDraft {
  return { id, method: '', amount: '', tenderedAmount: '' }
}

export function CheckoutDialog({
  organizationId,
  branchId,
  registerId,
  shiftId,
  sale,
  activeItemCount,
  operationalParentsActive,
  locale,
  currency,
  onClose,
  onCompleted,
  onReconcile,
}: Props) {
  const [initialSaleVersion] = useState(sale.version)
  const saleChanged = initialSaleVersion !== sale.version
  const zeroTotal = parseExactMoney(sale.totalAmount) === 0n
  const [payments, setPayments] = useState<PaymentDraft[]>(() => zeroTotal ? [] : [paymentRow(1)])
  const [nextId, setNextId] = useState(2)
  const [submitError, setSubmitError] = useState('')
  useEffect(() => {
    if (saleChanged) {
      onReconcile(sale.id, 'La venta cambió. Revisa el total antes de volver a cobrar.', false, false)
    }
  }, [onReconcile, sale.id, saleChanged])

  const checkoutMutation = useMutation({
    mutationFn: (instructions: Array<{ method: PaymentMethod; amount: string; tenderedAmount?: string }>) => checkoutSale(
      organizationId,
      branchId,
      registerId,
      shiftId,
      sale.id,
      { expectedSaleVersion: sale.version, payments: instructions },
    ),
    onSuccess: onCompleted,
    onError: (error) => {
      const message = checkoutErrorMessage(error)
      if (isRecoverableCheckoutError(error)) {
        const code = errorCode(error)
        onReconcile(
          sale.id,
          message,
          ['BRANCH_INACTIVE', 'CASH_REGISTER_INACTIVE', 'CASH_SHIFT_NOT_OPEN'].includes(code ?? ''),
          ['SALE_ALREADY_COMPLETED', 'SALE_ALREADY_CANCELLED', 'SALE_NOT_OPEN'].includes(code ?? ''),
        )
      } else {
        setSubmitError(message)
      }
    },
  })

  const total = parseExactMoney(sale.totalAmount)
  const parsedAmounts = payments.map((payment) => parseExactMoney(payment.amount))
  const applied = parsedAmounts.reduce<bigint>(
    (sum, amount) => amount !== null && amount > 0n ? addExactMoney(sum, amount) : sum,
    0n,
  )
  const remaining = total === null ? null : subtractExactMoney(total, applied)
  const remainingForPayment = (index: number) => {
    if (total === null) return null
    const appliedElsewhere = parsedAmounts.reduce<bigint>(
      (sum, amount, currentIndex) => currentIndex !== index && amount !== null && amount > 0n
        ? addExactMoney(sum, amount)
        : sum,
      0n,
    )
    return subtractExactMoney(total, appliedElsewhere)
  }
  const cashCount = payments.filter((payment) => payment.method === 'CASH').length

  function rowAmountError(index: number): string {
    const payment = payments[index]
    const amount = parsedAmounts[index]
    if (!payment.amount.trim()) return 'Ingresa el monto aplicado.'
    if (amount === null) return 'Usa un monto no negativo con hasta 4 decimales y máximo 15 dígitos enteros.'
    if (amount <= 0n) return 'El monto aplicado debe ser mayor que cero.'
    if (!payment.method) return 'Selecciona un método de pago.'
    return ''
  }

  function rowTenderError(index: number): string {
    const payment = payments[index]
    if (payment.method !== 'CASH') return ''
    const tendered = parseExactMoney(payment.tenderedAmount)
    const amount = parsedAmounts[index]
    if (!payment.tenderedAmount.trim()) return 'Ingresa el efectivo recibido.'
    if (tendered === null || tendered <= 0n) return 'Usa un monto de efectivo válido.'
    if (amount !== null && tendered < amount) return 'El efectivo recibido no puede ser menor al monto aplicado.'
    return ''
  }

  const rowErrors = payments.map((_, index) => rowAmountError(index))
  const tenderErrors = payments.map((_, index) => rowTenderError(index))
  const paymentsValid = total !== null
    && activeItemCount > 0
    && payments.length <= 10
    && (zeroTotal ? payments.length === 0 : payments.length > 0)
    && cashCount <= 1
    && rowErrors.every((error) => !error)
    && tenderErrors.every((error) => !error)
    && remaining !== null
    && compareExactMoney(remaining, 0n) === 0
  const canSubmit = !saleChanged && operationalParentsActive && activeItemCount > 0 && paymentsValid && !checkoutMutation.isPending

  function updatePayment(id: number, update: Partial<PaymentDraft>) {
    setSubmitError('')
    setPayments((current) => current.map((payment) => payment.id === id ? { ...payment, ...update } : payment))
  }

  function addPayment() {
    if (payments.length >= 10 || zeroTotal || checkoutMutation.isPending) return
    setPayments((current) => [...current, paymentRow(nextId)])
    setNextId((current) => current + 1)
  }

  function submit() {
    if (saleChanged || !canSubmit || total === null) return
    const instructions = payments.map((payment) => ({
      method: payment.method as PaymentMethod,
      amount: exactMoneyToDecimal(parseExactMoney(payment.amount)!),
      ...(payment.method === 'CASH'
        ? { tenderedAmount: exactMoneyToDecimal(parseExactMoney(payment.tenderedAmount)!) }
        : {}),
    }))
    checkoutMutation.mutate(instructions)
  }

  const appliedText = formatExactMoneyUnits(applied, locale, currency)
  const remainingText = remaining === null ? '—' : formatExactMoneyUnits(remaining, locale, currency)

  return (
    <Dialog
      open
      onClose={() => !checkoutMutation.isPending && onClose()}
      fullWidth
      maxWidth="sm"
      aria-labelledby="checkout-dialog-title"
    >
      <DialogTitle id="checkout-dialog-title">Cobrar venta</DialogTitle>
      <DialogContent dividers>
        <Stack spacing={2}>
          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: { xs: '1fr', sm: '1fr auto' },
              gap: 1.5,
              alignItems: 'center',
              p: 1.5,
              border: 1,
              borderColor: 'divider',
              borderRadius: 1,
              bgcolor: 'background.default',
            }}
          >
            <Box sx={{ minWidth: 0 }}>
              <Typography variant="body2" color="text.secondary">Total de la venta</Typography>
              <Typography variant="h4" aria-label="Total a cobrar">
                {total === null ? 'Monto de venta inválido' : formatExactMoneyUnits(total, locale, currency)}
              </Typography>
            </Box>
            <Box sx={{ textAlign: { xs: 'left', sm: 'right' } }}>
              <Typography variant="body2" color="text.secondary">Restante</Typography>
              <Typography
                variant="h5"
                aria-label="Restante por cubrir"
                sx={{
                  fontWeight: 700,
                  color: remaining !== null && remaining < 0n
                    ? 'error.main'
                    : remaining === 0n ? 'success.main' : 'text.primary',
                }}
              >
                {remainingText}
              </Typography>
            </Box>
          </Box>

          {!operationalParentsActive && (
            <Alert severity="warning">La sucursal, caja o turno ya no permite completar el cobro. Se actualizará el contexto operacional.</Alert>
          )}
          {activeItemCount === 0 && <Alert severity="warning">Agrega al menos un producto antes de cobrar.</Alert>}
          {zeroTotal && activeItemCount > 0 && (
            <Alert severity="info">Esta venta no requiere pagos. Puedes completarla con importe cero.</Alert>
          )}
          {submitError && <Alert role="alert" severity="error">{submitError}</Alert>}

          {!zeroTotal && payments.map((payment, index) => {
            const amount = parsedAmounts[index]
            const assignableAmount = remainingForPayment(index)
            const tendered = parseExactMoney(payment.tenderedAmount)
            const existingCashElsewhere = cashCount > (payment.method === 'CASH' ? 1 : 0)
            const canOfferCash = !existingCashElsewhere
            const change = payment.method === 'CASH' && amount !== null && tendered !== null && tendered >= amount
              ? subtractExactMoney(tendered, amount)
              : null
            return (
              <Box key={payment.id} sx={{ border: 1, borderColor: 'divider', borderRadius: 1, p: { xs: 1.5, sm: 2 } }}>
                <Stack spacing={1.5}>
                  <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1.5}>
                    <FormControl fullWidth size="small" error={!payment.method && Boolean(submitError)}>
                      <InputLabel id={`payment-method-${payment.id}`}>Método de pago</InputLabel>
                      <Select
                        labelId={`payment-method-${payment.id}`}
                        label="Método de pago"
                        value={payment.method}
                        onChange={(event) => {
                          const method = event.target.value as PaymentMethod | ''
                          updatePayment(payment.id, { method, ...(method === 'CARD' ? { tenderedAmount: '' } : {}) })
                        }}
                        disabled={checkoutMutation.isPending}
                      >
                        <MenuItem value="">Selecciona un método</MenuItem>
                        {canOfferCash && <MenuItem value="CASH">Efectivo</MenuItem>}
                        <MenuItem value="CARD">Tarjeta</MenuItem>
                      </Select>
                    </FormControl>
                    <TextField
                      label="Monto aplicado"
                      value={payment.amount}
                      onChange={(event) => updatePayment(payment.id, { amount: event.target.value })}
                      error={Boolean(rowErrors[index])}
                      helperText={rowErrors[index] || 'Usa punto decimal; hasta 4 decimales.'}
                      slotProps={{ htmlInput: { inputMode: 'decimal' } }}
                      disabled={checkoutMutation.isPending}
                      fullWidth
                    />
                  </Stack>
                  {assignableAmount !== null && assignableAmount > 0n && (
                    <Button
                      size="small"
                      disabled={checkoutMutation.isPending}
                      onClick={() => updatePayment(payment.id, { amount: exactMoneyToDecimal(assignableAmount) })}
                    >
                      Usar restante ({formatExactMoneyUnits(assignableAmount, locale, currency)})
                    </Button>
                  )}
                  {payment.method === 'CASH' && (
                    <TextField
                      label="Efectivo recibido"
                      value={payment.tenderedAmount}
                      onChange={(event) => updatePayment(payment.id, { tenderedAmount: event.target.value })}
                      error={Boolean(tenderErrors[index])}
                      helperText={tenderErrors[index] || 'Debe cubrir el monto aplicado.'}
                      slotProps={{ htmlInput: { inputMode: 'decimal' } }}
                      disabled={checkoutMutation.isPending}
                      fullWidth
                    />
                  )}
                  {change !== null && (
                    <Typography role="status" variant="body2">
                      Cambio estimado: {formatExactMoneyUnits(change, locale, currency)}
                    </Typography>
                  )}
                  {payments.length > 1 && (
                    <Button
                      color="inherit"
                      size="small"
                      disabled={checkoutMutation.isPending}
                      onClick={() => setPayments((current) => current.filter((item) => item.id !== payment.id))}
                      aria-label={`Quitar método de pago ${index + 1}`}
                    >
                      Quitar método
                    </Button>
                  )}
                </Stack>
              </Box>
            )
          })}

          {!zeroTotal && (
            <Button onClick={addPayment} disabled={payments.length >= 10 || checkoutMutation.isPending}>
              {payments.length >= 10 ? 'Máximo de 10 métodos' : 'Agregar método de pago'}
            </Button>
          )}

          <Box sx={{ borderTop: 1, borderColor: 'divider', pt: 1.5 }}>
            <Stack spacing={0.75}>
              <Stack direction="row" sx={{ justifyContent: 'space-between' }}>
                <Typography>Aplicado</Typography><Typography>{appliedText}</Typography>
              </Stack>
              {!zeroTotal && remaining !== null && remaining < 0n && (
                <Alert severity="error">El monto aplicado excede el total.</Alert>
              )}
            </Stack>
          </Box>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', justifyContent: 'space-between', px: 2, py: 1.5 }}>
        <Button onClick={onClose} disabled={checkoutMutation.isPending}>Volver al carrito</Button>
        <Button
          variant="contained"
          onClick={submit}
          disabled={!canSubmit}
        >
          {checkoutMutation.isPending ? 'Procesando cobro…' : 'Completar venta'}
        </Button>
      </DialogActions>
      <FormHelperText sx={{ px: 2, pb: 1 }}>
        Tarjeta registra el método de pago; no procesa ni autoriza una tarjeta.
      </FormHelperText>
    </Dialog>
  )
}
