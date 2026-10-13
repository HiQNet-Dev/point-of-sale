/**
 * Query keys del agregado Organization. Con clave solo por organizationId:
 * la política de stock negativo es organization-scoped y no tiene contexto de branch.
 */
export const organizationKeys = {
  all: ['organization'] as const,
  detail: (organizationId: string) => [...organizationKeys.all, 'detail', organizationId] as const,
}
