export const posSalesKeys = {
  openSales: (
    organizationId: string,
    branchId: string,
    cashRegisterId: string,
    cashShiftId: string,
    page: number,
  ) => ['sales', organizationId, branchId, cashRegisterId, cashShiftId, 'OPEN', page] as const,
  sale: (
    organizationId: string,
    branchId: string,
    cashRegisterId: string,
    cashShiftId: string,
    saleId: string,
  ) => ['sale', organizationId, branchId, cashRegisterId, cashShiftId, saleId] as const,
  categories: (organizationId: string, branchId: string, page: number) =>
    ['pos-catalog', organizationId, branchId, 'categories', page] as const,
  products: (
    organizationId: string,
    branchId: string,
    page: number,
    q: string,
    categoryId: string | null,
  ) => ['pos-catalog', organizationId, branchId, 'products', page, q, categoryId] as const,
  configuration: (organizationId: string, branchId: string, productId: string) =>
    ['pos-catalog', organizationId, branchId, 'product-configuration', productId] as const,
}
