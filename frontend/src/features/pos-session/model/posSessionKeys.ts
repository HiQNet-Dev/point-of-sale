export const posSessionKeys = {
  branches: (page: number, size: number, q = '') => ['auth', 'me', 'branches', page, size, q] as const,
  registers: (organizationId: string, branchId: string, page: number, size: number, q = '') =>
    ['cash-registers', organizationId, branchId, page, size, q] as const,
  openShifts: (organizationId: string, branchId: string, cashRegisterId: string) =>
    ['cash-shifts', organizationId, branchId, cashRegisterId, 'OPEN'] as const,
}
