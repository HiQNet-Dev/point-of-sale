/**
 * Query keys for the organization aggregate. Keyed by organizationId only —
 * the negative-stock policy is organization-scoped and has no branch context.
 */
export const organizationKeys = {
  all: ['organization'] as const,
  detail: (organizationId: string) => [...organizationKeys.all, 'detail', organizationId] as const,
}
