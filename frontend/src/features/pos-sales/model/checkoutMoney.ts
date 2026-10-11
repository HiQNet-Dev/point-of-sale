const MONEY_FACTOR = 10_000n
const MAX_WHOLE = 999_999_999_999_999n
const DECIMAL_INPUT = /^(\d+)(?:\.(\d{1,4}))?$/

export function parseExactMoney(value: string): bigint | null {
  const match = DECIMAL_INPUT.exec(value.trim())
  if (!match) return null

  const wholeText = match[1].replace(/^0+(?=\d)/, '')
  if (wholeText.length > 15) return null
  const whole = BigInt(wholeText)
  if (whole > MAX_WHOLE) return null

  const fraction = (match[2] ?? '').padEnd(4, '0')
  return whole * MONEY_FACTOR + BigInt(fraction || '0')
}

export function addExactMoney(left: bigint, right: bigint): bigint {
  return left + right
}

export function subtractExactMoney(left: bigint, right: bigint): bigint {
  return left - right
}

export function compareExactMoney(left: bigint, right: bigint): number {
  return left < right ? -1 : left > right ? 1 : 0
}

export function exactMoneyToDecimal(value: bigint): string {
  const negative = value < 0n
  const absolute = negative ? -value : value
  const whole = absolute / MONEY_FACTOR
  const fraction = (absolute % MONEY_FACTOR).toString().padStart(4, '0').replace(/0+$/, '')
  return `${negative ? '-' : ''}${whole}${fraction ? `.${fraction}` : ''}`
}

export function formatExactMoney(value: string, locale: string, currency: string): string {
  const units = parseExactMoney(value)
  if (units === null) throw new RangeError('Invalid exact money value')
  return formatExactMoneyUnits(units, locale, currency)
}

export function formatExactMoneyUnits(units: bigint, locale: string, currency: string): string {
  const negative = units < 0n
  const absolute = negative ? -units : units
  const whole = absolute / MONEY_FACTOR
  const rawFraction = (absolute % MONEY_FACTOR).toString().padStart(4, '0').replace(/0+$/, '')
  const currencyFormatter = new Intl.NumberFormat(locale, {
    style: 'currency',
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  })
  const minimumFractionDigits = new Intl.NumberFormat(locale, { style: 'currency', currency })
    .resolvedOptions().minimumFractionDigits ?? 0
  const fraction = rawFraction.padEnd(minimumFractionDigits, '0')
  const parts = currencyFormatter.formatToParts(negative
    ? whole === 0n
      ? -1n
      : -whole
    : whole)
  if (negative && whole === 0n) {
    const integerPart = parts.find((part) => part.type === 'integer')
    if (integerPart) integerPart.value = '0'
  }

  if (!fraction) return parts.map((part) => part.value).join('')

  const decimal = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 })
    .formatToParts(1.1).find((part) => part.type === 'decimal')?.value ?? '.'
  const digitFormatter = new Intl.NumberFormat(locale, { useGrouping: false })
  const localizedFraction = [...fraction]
    .map((digit) => digitFormatter.format(BigInt(digit)))
    .join('')
  let lastIntegerPart = -1
  parts.forEach((part, index) => {
    if (part.type === 'integer' || part.type === 'group') lastIntegerPart = index
  })
  parts.splice(lastIntegerPart + 1, 0, { type: 'decimal', value: decimal }, { type: 'fraction', value: localizedFraction })
  return parts.map((part) => part.value).join('')
}
