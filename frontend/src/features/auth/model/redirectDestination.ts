export function postLoginDestination(state: unknown): string {
  const from = (state as { from?: { pathname?: string; search?: string; hash?: string } } | null)?.from
  const pathname = from?.pathname
  if (!from || !pathname?.startsWith('/') || pathname.startsWith('//')) return '/'
  return `${pathname}${from.search ?? ''}${from.hash ?? ''}`
}
