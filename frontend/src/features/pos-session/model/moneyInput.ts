const COMPLETE_AMOUNT = /^\d+(?:\.\d{1,4})?$/

export function validateAmount(value: string): string | null {
  if (!value.trim()) return 'Ingresa un monto.'
  if (!COMPLETE_AMOUNT.test(value)) return 'Usa un monto no negativo con hasta 4 decimales.'
  return null
}

export function formatMoney(amount: number, locale: string, currency: string): string {
  return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amount)
}

export function formatTimestamp(value: string, locale: string, timezone: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: timezone,
  }).format(new Date(value))
}
