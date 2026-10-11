import { Alert, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle, FormControl, FormControlLabel, FormLabel, Paper, Radio, RadioGroup, Skeleton, Stack, TextField, Typography } from '@mui/material'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { UseQueryResult } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'
import type { Branch, CashRegister, CashShift, PageResponse } from '../../pos-session/api/posSessionApi'
import { posSessionKeys } from '../../pos-session/model/posSessionKeys'
import { ApiRequestError } from '../../../shared/api/client'
import {
  addSaleItem,
  cancelSale,
  createSale,
  getProductConfiguration,
  getSale,
  listOpenSales,
  listOperationalCategories,
  listOperationalProducts,

  removeSaleItem,
  updateSaleItem,
} from '../api/posSalesApi'
import type { Category, ModifierGroup, Product, ProductConfiguration, SaleDetail, SaleItem } from '../api/posSalesApi'
import { posSalesKeys } from '../model/posSalesKeys'

type Props = {
  organizationId: string
  branch: Branch
  register: CashRegister
  shift: CashShift
  locale: string
  currency: string
}

type DialogMode = 'add' | 'edit' | null

function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

function errorMessage(error: unknown, fallback: string): string {
  if (errorCode(error) === 'FORBIDDEN' || (error instanceof ApiRequestError && error.status === 403)) {
    return 'No tienes permiso para realizar esta operación.'
  }
  if (errorCode(error) === 'CONCURRENT_MODIFICATION') {
    return 'La venta cambió en otra operación. Se actualizará el carrito; revisa los datos antes de continuar.'
  }
  if (errorCode(error) === 'BRANCH_INACTIVE') return 'La sucursal está inactiva. Se actualizará el contexto operacional.'
  if (errorCode(error) === 'CASH_REGISTER_INACTIVE') return 'La caja está inactiva. Se actualizará el contexto operacional.'
  if (errorCode(error) === 'CASH_SHIFT_NOT_OPEN' || errorCode(error) === 'CASH_SHIFT_ALREADY_CLOSED') {
    return 'El turno ya no está abierto. Se actualizará el contexto operacional.'
  }
  if (saleStateConflict(error)) return 'La venta ya no está abierta. Se actualizará la lista de ventas.'
  if (errorCode(error) === 'SALE_ITEM_ALREADY_REMOVED') return 'El producto ya se había quitado. Se actualizará el carrito.'
  return fallback
}

function formatMoney(amount: number, locale: string, currency: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amount)
  } catch {
    return `${amount.toLocaleString(locale)} ${currency}`
  }
}

function shortSaleId(id: string): string {
  return id.slice(0, 8).toUpperCase()
}

function activeOptions(group: ModifierGroup) {
  return group.options.filter((option) => option.status === 'ACTIVE')
}

function requiredInactiveGroup(configuration: ProductConfiguration | undefined): boolean {
  return Boolean(configuration?.groups.some((group) => group.status === 'INACTIVE' && group.minSelections > 0))
}

function saleStateConflict(error: unknown): boolean {
  return ['SALE_NOT_OPEN', 'SALE_ALREADY_CANCELLED', 'SALE_ALREADY_COMPLETED'].includes(errorCode(error) ?? '')
}

function modifierConfigurationConflict(error: unknown): boolean {
  return errorCode(error) === 'PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE'
}

export function SalesWorkspace({ organizationId, branch, register, shift, locale, currency }: Props) {
  const queryClient = useQueryClient()
  const [salePage, setSalePage] = useState(0)
  const [productPage, setProductPage] = useState(0)
  const [categoryPage, setCategoryPage] = useState(0)
  const [categoryId, setCategoryId] = useState('')
  const [searchInput, setSearchInput] = useState('')
  const [search, setSearch] = useState('')
  const [selectedSaleId, setSelectedSaleId] = useState('')
  const [dismissedSaleId, setDismissedSaleId] = useState('')
  const [selectedProductId, setSelectedProductId] = useState('')
  const [dialogMode, setDialogMode] = useState<DialogMode>(null)
  const [editingItem, setEditingItem] = useState<SaleItem | null>(null)
  const [quantityInput, setQuantityInput] = useState<string | null>('1')
  const [optionSelectionOverride, setOptionSelectionOverride] = useState<string[] | null>([])
  const [itemToRemove, setItemToRemove] = useState<SaleItem | null>(null)
  const [cancelDialogOpen, setCancelDialogOpen] = useState(false)
  const [operationMessage, setOperationMessage] = useState<string | null>(null)

  const branchId = branch.id
  const registerId = register.id
  const shiftId = shift.id
  const operationalParentsActive = branch.status === 'ACTIVE'
    && register.status === 'ACTIVE'
    && shift.status === 'OPEN'

  const salesPrefix = useMemo(
    () => ['sales', organizationId, branchId, registerId, shiftId, 'OPEN'] as const,
    [organizationId, branchId, registerId, shiftId],
  )
  const listKey = posSalesKeys.openSales(organizationId, branchId, registerId, shiftId, salePage)
  const salesQuery = useQuery({
    queryKey: listKey,
    queryFn: () => listOpenSales(organizationId, branchId, registerId, shiftId, salePage),
  })
  const autoSelectedSaleId = !selectedSaleId && salePage === 0 && salesQuery.data?.items.length === 1
    && salesQuery.data.items[0].id !== dismissedSaleId
    ? salesQuery.data.items[0].id
    : ''
  const activeSaleId = selectedSaleId || autoSelectedSaleId
  const selectedSaleKey = activeSaleId
    ? posSalesKeys.sale(organizationId, branchId, registerId, shiftId, activeSaleId)
    : ['sale', organizationId, branchId, registerId, shiftId, ''] as const
  const saleQuery = useQuery({
    queryKey: selectedSaleKey,
    queryFn: () => getSale(organizationId, branchId, registerId, shiftId, activeSaleId),
    enabled: Boolean(activeSaleId),
  })
  const saleDetail = saleQuery.data
  const saleStatus = saleDetail?.sale.status
  const saleIsOpen = saleStatus === 'OPEN'
  const canEditSale = operationalParentsActive && Boolean(saleIsOpen)
  const catalogEnabled = canEditSale

  const categoriesQuery = useQuery({
    queryKey: posSalesKeys.categories(organizationId, branchId, categoryPage),
    queryFn: () => listOperationalCategories(organizationId, branchId, categoryPage),
    enabled: catalogEnabled,
  })
  const productsQuery = useQuery({
    queryKey: posSalesKeys.products(organizationId, branchId, productPage, search, categoryId || null),
    queryFn: () => listOperationalProducts(organizationId, branchId, {
      page: productPage,
      q: search,
      categoryId: categoryId || null,
    }),
    enabled: catalogEnabled,
  })
  const configurationKey = selectedProductId
    ? posSalesKeys.configuration(organizationId, branchId, selectedProductId)
    : ['pos-catalog', organizationId, branchId, 'product-configuration', ''] as const
  const configurationQuery = useQuery({
    queryKey: configurationKey,
    queryFn: () => getProductConfiguration(organizationId, branchId, selectedProductId),
    enabled: Boolean(selectedProductId && dialogMode),
  })
  const configuration = configurationQuery.data

  useEffect(() => {
    if (errorCode(configurationQuery.error) === 'PRODUCT_INACTIVE') {
      void queryClient.invalidateQueries({ queryKey: ['pos-catalog', organizationId, branchId, 'products'] })
    }
  }, [branchId, configurationQuery.error, organizationId, queryClient])

  useEffect(() => {
    const nextSearch = searchInput.trim()
    if (nextSearch === search) return
    const timer = window.setTimeout(() => {
      setSearch(nextSearch)
      setProductPage(0)
    }, 300)
    return () => window.clearTimeout(timer)
  }, [search, searchInput])

  useEffect(() => {
    if (saleStatus && saleStatus !== 'OPEN') {
      void queryClient.invalidateQueries({ queryKey: salesPrefix })
    }
  }, [queryClient, saleStatus, salesPrefix])

  const validHistoricalOptionIds = useMemo(() => {
    if (!editingItem || !configuration) return []
    const activeIds = new Set(configuration.groups
      .filter((group) => group.status === 'ACTIVE')
      .flatMap((group) => activeOptions(group).map((option) => option.id)))
    return editingItem.modifiers.map((modifier) => modifier.modifierOptionId).filter((id) => activeIds.has(id))
  }, [configuration, editingItem])
  const selectedOptionIds = optionSelectionOverride ?? validHistoricalOptionIds
  const staleEditConfiguration = Boolean(
    dialogMode === 'edit'
    && editingItem
    && configuration
    && optionSelectionOverride === null
    && validHistoricalOptionIds.length !== editingItem.modifiers.length,
  )
  const quantity = quantityInput ?? (dialogMode === 'edit' && editingItem ? String(editingItem.quantity) : '1')

  const invalidateSale = async (saleId = activeSaleId) => {
    if (saleId) {
      await queryClient.invalidateQueries({
        queryKey: posSalesKeys.sale(organizationId, branchId, registerId, shiftId, saleId),
      })
    }
    await queryClient.invalidateQueries({ queryKey: salesPrefix })
  }

  const refreshOperationalContext = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['auth', 'me', 'branches'] }),
      queryClient.invalidateQueries({ queryKey: ['cash-registers', organizationId, branchId] }),
      queryClient.invalidateQueries({ queryKey: posSessionKeys.openShifts(organizationId, branchId, registerId) }),
      invalidateSale(),
    ])
  }

  const createMutation = useMutation({
    mutationFn: () => createSale(organizationId, branchId, registerId, shiftId),
    onSuccess: async (sale) => {
      setOperationMessage(null)
      setSalePage(0)
      setSelectedSaleId(sale.id)
      await queryClient.invalidateQueries({ queryKey: salesPrefix })
    },
    onError: async (error) => {
      setOperationMessage(errorMessage(error, 'No se pudo iniciar una venta.'))
      if (['BRANCH_INACTIVE', 'CASH_REGISTER_INACTIVE', 'CASH_SHIFT_NOT_OPEN'].includes(errorCode(error) ?? '')) {
        await refreshOperationalContext()
      }
    },
  })

  const addMutation = useMutation({
    mutationFn: (input: { productId: string; quantity: number; modifierOptionIds: string[] }) => addSaleItem(
      organizationId, branchId, registerId, shiftId, activeSaleId,
      { ...input, expectedSaleVersion: saleDetail!.sale.version },
    ),
    onSuccess: async () => {
      setDialogMode(null)
      setSelectedProductId('')
      setOperationMessage(null)
      await invalidateSale()
    },
    onError: async (error) => {
      setOperationMessage(errorMessage(error, itemErrorMessage(error)))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION' || saleStateConflict(error)) {
        if (saleStateConflict(error)) {
          setDialogMode(null)
          setSelectedProductId('')
        }
        await invalidateSale()
      }
      if (modifierConfigurationConflict(error)) {
        await queryClient.invalidateQueries({ queryKey: configurationKey })
      }
      if (errorCode(error) === 'PRODUCT_INACTIVE') {
        setDialogMode(null)
        setSelectedProductId('')
        setEditingItem(null)
        await queryClient.invalidateQueries({ queryKey: ['pos-catalog', organizationId, branchId, 'products'] })
      }
      if (['BRANCH_INACTIVE', 'CASH_REGISTER_INACTIVE', 'CASH_SHIFT_NOT_OPEN'].includes(errorCode(error) ?? '')) {
        await refreshOperationalContext()
      }
    },
  })

  const updateMutation = useMutation({
    mutationFn: (input: { quantity: number; modifierOptionIds: string[] }) => updateSaleItem(
      organizationId, branchId, registerId, shiftId, activeSaleId, editingItem!.id,
      { ...input, expectedSaleVersion: saleDetail!.sale.version },
    ),
    onSuccess: async () => {
      setDialogMode(null)
      setEditingItem(null)
      setSelectedProductId('')
      setOperationMessage(null)
      await invalidateSale()
    },
    onError: async (error) => {
      setOperationMessage(errorMessage(error, itemErrorMessage(error)))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION' || saleStateConflict(error)) {
        if (saleStateConflict(error)) {
          setDialogMode(null)
          setSelectedProductId('')
          setEditingItem(null)
        }
        await invalidateSale()
      }
      if (modifierConfigurationConflict(error)) {
        await queryClient.invalidateQueries({ queryKey: configurationKey })
      }
      if (errorCode(error) === 'PRODUCT_INACTIVE') {
        setDialogMode(null)
        setSelectedProductId('')
        setEditingItem(null)
        await queryClient.invalidateQueries({ queryKey: ['pos-catalog', organizationId, branchId, 'products'] })
      }
      if (['BRANCH_INACTIVE', 'CASH_REGISTER_INACTIVE', 'CASH_SHIFT_NOT_OPEN'].includes(errorCode(error) ?? '')) {
        await refreshOperationalContext()
      }
    },
  })

  const removeMutation = useMutation({
    mutationFn: (item: SaleItem) => removeSaleItem(
      organizationId, branchId, registerId, shiftId, activeSaleId, item.id, saleDetail!.sale.version,
    ),
    onSuccess: async () => {
      setItemToRemove(null)
      setOperationMessage(null)
      await invalidateSale()
    },
    onError: async (error) => {
      setOperationMessage(errorMessage(error, 'No se pudo quitar el producto.'))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION' || saleStateConflict(error)
        || errorCode(error) === 'SALE_ITEM_ALREADY_REMOVED') {
        setItemToRemove(null)
        await invalidateSale()
      }
      if (['BRANCH_INACTIVE', 'CASH_REGISTER_INACTIVE', 'CASH_SHIFT_NOT_OPEN'].includes(errorCode(error) ?? '')) {
        await refreshOperationalContext()
      }
    },
  })

  const cancelMutation = useMutation({
    mutationFn: () => cancelSale(
      organizationId, branchId, registerId, shiftId, activeSaleId, saleDetail!.sale.version,
    ),
    onSuccess: async () => {
      setCancelDialogOpen(false)
      setDismissedSaleId(activeSaleId)
      setSelectedSaleId('')
      setOperationMessage('Venta cancelada. La venta permanece en el historial.')
      await invalidateSale(activeSaleId)
    },
    onError: async (error) => {
      setOperationMessage(errorMessage(error, 'No se pudo cancelar la venta.'))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION' || saleStateConflict(error)) {
        setCancelDialogOpen(false)
        await invalidateSale()
      }
    },
  })

  function openAdd(product: Product) {
    setOperationMessage(null)
    setEditingItem(null)
    setOptionSelectionOverride([])
    setQuantityInput('1')
    setSelectedProductId(product.id)
    setDialogMode('add')
  }

  function openEdit(item: SaleItem) {
    setOperationMessage(null)
    setEditingItem(item)
    setOptionSelectionOverride(null)
    setQuantityInput(null)
    setSelectedProductId(item.productId)
    setDialogMode('edit')
  }

  function closeConfiguration() {
    if (addMutation.isPending || updateMutation.isPending) return
    setDialogMode(null)
    setSelectedProductId('')
    setEditingItem(null)
  }

  function toggleOption(group: ModifierGroup, optionId: string, checked: boolean) {
    setOptionSelectionOverride((currentOverride) => {
      const current = currentOverride ?? selectedOptionIds
      if (group.maxSelections === 1) {
        return checked ? [...current.filter((id) => !group.options.some((option) => option.id === id)), optionId]
          : current.filter((id) => id !== optionId)
      }
      if (!checked) return current.filter((id) => id !== optionId)
      if (current.includes(optionId)) return current
      const count = group.options.filter((option) => current.includes(option.id)).length
      if (count >= group.maxSelections) return current
      return [...current, optionId]
    })
  }

  const invalidSelection = useMemo(() => {
    if (!configuration) return true
    if (requiredInactiveGroup(configuration)) return true
    return configuration.groups.some((group) => {
      if (group.status !== 'ACTIVE') return false
      const count = group.options.filter((option) => option.status === 'ACTIVE' && selectedOptionIds.includes(option.id)).length
      return count < group.minSelections || count > group.maxSelections
    })
  }, [configuration, selectedOptionIds])

  function submitItem() {
    const parsedQuantity = Number(quantity)
    if (!Number.isInteger(parsedQuantity) || parsedQuantity < 1 || parsedQuantity > 1_000_000 || !configuration || invalidSelection) return
    const payload = { quantity: parsedQuantity, modifierOptionIds: selectedOptionIds }
    if (dialogMode === 'edit') updateMutation.mutate(payload)
    else if (dialogMode === 'add') addMutation.mutate({ ...payload, productId: selectedProductId })
  }

  const pageSales = salesQuery.data
  const categories = categoriesQuery.data?.items ?? []
  const products = productsQuery.data?.items ?? []
  const configLoadError = configurationQuery.error
  const configurationHasUnavailableState = requiredInactiveGroup(configuration)

  function itemErrorMessage(error: unknown): string {
    switch (errorCode(error)) {
      case 'INVALID_MODIFIER_SELECTION': return 'Revisa las opciones: la selección no cumple las reglas del producto.'
      case 'PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE': return 'La configuración cambió y ya no permite agregar este producto.'
      case 'PRODUCT_INACTIVE': return 'El producto dejó de estar disponible. Se actualizará el catálogo.'
      case 'NEGATIVE_EFFECTIVE_PRICE': return 'La selección produce un precio no válido. Cambia las opciones.'
      default: return 'No se pudo guardar el producto. Revisa el carrito actualizado.'
    }
  }

  return (
    <Paper variant="outlined" sx={{ p: { xs: 2, sm: 3 }, mt: 3 }}>
      <Stack spacing={2.5}>
        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1.5} sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}>
          <Box>
            <Typography component="h2" variant="h5">Ventas abiertas</Typography>
            <Typography color="text.secondary" variant="body2">El carrito se recupera desde la venta guardada en el servidor.</Typography>
          </Box>
          <Button
            variant="contained"
            onClick={() => createMutation.mutate()}
            disabled={!operationalParentsActive || createMutation.isPending || salesQuery.isLoading}
          >
            {createMutation.isPending ? 'Iniciando…' : 'Nueva venta'}
          </Button>
        </Stack>

        {operationMessage && !activeSaleId && <Alert role="status" severity={operationMessage.includes('permiso') ? 'warning' : 'info'}>{operationMessage}</Alert>}
        {salesQuery.isLoading && <Skeleton variant="rounded" height={54} />}
        {salesQuery.error && <Alert severity="warning">No se pudieron consultar las ventas abiertas. {errorMessage(salesQuery.error, 'Intenta actualizar.')}</Alert>}
        {salesQuery.data?.totalElements === 0 && <Alert severity="info">No hay ventas abiertas.</Alert>}
        {pageSales && pageSales.totalElements > 1 && (
          <Stack direction="row" spacing={1} sx={{ alignItems: 'center', flexWrap: 'wrap' }}>
            {pageSales.items.map((sale) => (
              <Button
                key={sale.id}
                variant={sale.id === activeSaleId ? 'contained' : 'outlined'}
                onClick={() => { setSelectedSaleId(sale.id); setOperationMessage(null) }}
                aria-label={`Reanudar venta ${shortSaleId(sale.id)}`}
              >
                Reanudar {shortSaleId(sale.id)} · {formatMoney(sale.totalAmount, locale, currency)}
              </Button>
            ))}
          </Stack>
        )}
        {pageSales && pageSales.totalElements > 1 && (
          <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
            <Button disabled={salePage === 0} onClick={() => setSalePage((page) => page - 1)}>Ventas anteriores</Button>
            <Typography variant="body2">Página {salePage + 1} de {pageSales.totalPages}</Typography>
            <Button disabled={salePage + 1 >= pageSales.totalPages} onClick={() => setSalePage((page) => page + 1)}>Más ventas</Button>
          </Stack>
        )}

        {activeSaleId && (
          <SaleCartWorkspace
            organizationId={organizationId}

            locale={locale}
            currency={currency}
            saleDetail={saleDetail}
            saleLoading={saleQuery.isLoading}
            saleError={saleQuery.error}
            catalogEnabled={catalogEnabled}
            categories={categories}
            categoriesQuery={categoriesQuery}
            categoryPage={categoryPage}
            setCategoryPage={setCategoryPage}
            categoryId={categoryId}
            setCategoryId={(value) => { setCategoryId(value); setProductPage(0) }}
            products={products}
            productsQuery={productsQuery}
            productPage={productPage}
            setProductPage={setProductPage}
            searchInput={searchInput}
            setSearchInput={setSearchInput}
            operationMessage={operationMessage}
            onOpenAdd={openAdd}
            onOpenEdit={openEdit}
            onRemove={setItemToRemove}
            onCancel={() => setCancelDialogOpen(true)}
            onExitSale={() => {
              setDismissedSaleId(activeSaleId)
              setSelectedSaleId('')
              void invalidateSale(activeSaleId)
            }}
            onRefresh={() => void invalidateSale()}
            pending={addMutation.isPending || updateMutation.isPending || removeMutation.isPending || cancelMutation.isPending}
          />
        )}
      </Stack>

      <Dialog open={Boolean(dialogMode)} onClose={closeConfiguration} fullWidth maxWidth="sm" aria-labelledby="product-config-title">
        <DialogTitle id="product-config-title">{dialogMode === 'edit' ? 'Editar producto' : 'Agregar producto'}</DialogTitle>
        <DialogContent dividers>
          {configurationQuery.isLoading && <Stack spacing={1}><Skeleton /><Skeleton /><Skeleton /></Stack>}
          {configLoadError && (
            <Alert
              severity="warning"
              action={errorCode(configLoadError) === 'PRODUCT_INACTIVE' ? (
                <Button color="inherit" onClick={() => {
                  setDialogMode(null)
                  setSelectedProductId('')
                  void queryClient.invalidateQueries({ queryKey: ['pos-catalog', organizationId, branchId, 'products'] })
                }}>Actualizar catálogo</Button>
              ) : undefined}
            >
              {errorCode(configLoadError) === 'PRODUCT_INACTIVE'
                ? 'Este producto dejó de estar disponible. Cierra la configuración y actualiza el catálogo.'
                : errorMessage(configLoadError, 'No se pudo cargar la configuración actual del producto.')}
            </Alert>
          )}
          {configuration && (
            <Stack spacing={2}>
              <Box>
                <Typography variant="h6">{configuration.product.name}</Typography>
                <Typography color="text.secondary" variant="body2">SKU {configuration.product.sku} · {formatMoney(configuration.product.unitPrice, locale, currency)}</Typography>
              </Box>
              {configurationHasUnavailableState && (
                <Alert severity="warning">Producto temporalmente no disponible por su configuración.</Alert>
              )}
              {staleEditConfiguration && (
                <Alert
                  severity="warning"
                  action={<Button color="inherit" onClick={() => setOptionSelectionOverride(validHistoricalOptionIds)}>Usar opciones activas</Button>}
                >
                  La configuración cambió. Confirma las opciones activas antes de guardar.
                </Alert>
              )}
              <TextField
                label="Cantidad"
                type="number"
                value={quantity}
                onChange={(event) => setQuantityInput(event.target.value)}
                slotProps={{ htmlInput: { min: 1, max: 1_000_000, step: 1, inputMode: 'numeric' } }}
                fullWidth
              />
              {configuration.groups.map((group) => <ModifierGroupControl
                key={group.id}
                group={group}
                selectedOptionIds={selectedOptionIds}
                locale={locale}
                currency={currency}
                onToggle={(id, checked) => toggleOption(group, id, checked)}
              />)}
            </Stack>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={closeConfiguration} disabled={addMutation.isPending || updateMutation.isPending}>Volver</Button>
          <Button
            variant="contained"

            onClick={submitItem}
            disabled={!canEditSale || !configuration || configurationQuery.isError || configurationQuery.isLoading
              || invalidSelection || staleEditConfiguration || !Number.isInteger(Number(quantity))
              || Number(quantity) < 1 || Number(quantity) > 1_000_000
              || addMutation.isPending || updateMutation.isPending}
          >
            {addMutation.isPending || updateMutation.isPending ? 'Guardando…' : dialogMode === 'edit' ? 'Guardar cambios' : 'Agregar a la venta'}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={Boolean(itemToRemove)} onClose={() => !removeMutation.isPending && setItemToRemove(null)}>
        <DialogTitle>Quitar producto</DialogTitle>
        <DialogContent><Typography>¿Quieres quitar {itemToRemove?.productNameSnapshot} de esta venta?</Typography></DialogContent>
        <DialogActions>
          <Button onClick={() => setItemToRemove(null)} disabled={removeMutation.isPending}>Conservar</Button>
          <Button color="error" onClick={() => itemToRemove && removeMutation.mutate(itemToRemove)} disabled={removeMutation.isPending}>
            {removeMutation.isPending ? 'Quitando…' : 'Quitar'}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={cancelDialogOpen} onClose={() => !cancelMutation.isPending && setCancelDialogOpen(false)}>
        <DialogTitle>Cancelar venta</DialogTitle>
        <DialogContent><Typography>La venta quedará cancelada y ya no podrá modificarse. Se conservará en el historial.</Typography></DialogContent>
        <DialogActions>
          <Button onClick={() => setCancelDialogOpen(false)} disabled={cancelMutation.isPending}>Volver</Button>
          <Button color="error" onClick={() => cancelMutation.mutate()} disabled={cancelMutation.isPending || !saleDetail || saleDetail.sale.status !== 'OPEN'}>
            {cancelMutation.isPending ? 'Cancelando…' : 'Cancelar venta'}
          </Button>
        </DialogActions>
      </Dialog>
    </Paper>
  )
}

type CartWorkspaceProps = {
  organizationId: string
  locale: string
  currency: string
  saleDetail: SaleDetail | undefined
  saleLoading: boolean
  saleError: Error | null
  catalogEnabled: boolean
  categories: Category[]
  categoriesQuery: UseQueryResult<PageResponse<Category>, Error>
  categoryPage: number
  setCategoryPage: (page: number) => void
  categoryId: string
  setCategoryId: (value: string) => void
  products: Product[]
  productsQuery: UseQueryResult<PageResponse<Product>, Error>
  productPage: number
  setProductPage: (page: number) => void
  searchInput: string
  setSearchInput: (value: string) => void
  operationMessage: string | null
  onOpenAdd: (product: Product) => void
  onOpenEdit: (item: SaleItem) => void
  onRemove: (item: SaleItem) => void
  onCancel: () => void
  onExitSale: () => void
  onRefresh: () => void
  pending: boolean
}

function SaleCartWorkspace(props: CartWorkspaceProps) {
  const {
    locale, currency, saleDetail, saleLoading, saleError, catalogEnabled,
    categories, categoriesQuery, categoryPage, setCategoryPage, categoryId, setCategoryId,
    products, productsQuery, productPage, setProductPage, searchInput, setSearchInput,
    operationMessage, onOpenAdd, onOpenEdit, onRemove, onCancel, onExitSale, onRefresh, pending,
  } = props
  const activeItems = saleDetail?.items.filter((item) => item.status === 'ACTIVE') ?? []
  const removedItems = saleDetail?.items.filter((item) => item.status === 'REMOVED') ?? []
  const showCatalog = catalogEnabled && saleDetail?.sale.status === 'OPEN'
  const saleErrorCode = errorCode(saleError)

  return (
    <Stack spacing={2}>
      {saleLoading && <Skeleton variant="rounded" height={140} />}
      {saleError && (
        <Alert severity="warning" action={<Button color="inherit" onClick={onRefresh}>Actualizar</Button>}>
          {saleErrorCode === 'FORBIDDEN' ? 'No tienes permiso para consultar esta venta.' : 'No se pudo cargar la venta.'}
        </Alert>
      )}
      {saleDetail && (
        <>
          {saleDetail.sale.status !== 'OPEN' && (
            <Alert severity="info" action={<Button color="inherit" onClick={onExitSale}>Volver a ventas</Button>}>
              Esta venta ya no está abierta y se muestra solo como referencia.
            </Alert>
          )}
          {saleDetail.sale.status === 'OPEN' && !showCatalog && (
            <Alert severity="warning">La sucursal, caja o turno ya no permite agregar o cambiar productos. Puedes revisar y cancelar la venta.</Alert>
          )}
          {operationMessage && <Alert role="status" severity={operationMessage.includes('permiso') ? 'warning' : 'info'}>{operationMessage}</Alert>}
          {operationMessage?.includes('contexto operacional') && (
            <Button onClick={onRefresh}>Actualizar contexto</Button>
          )}

          <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', lg: 'minmax(0, 1.4fr) minmax(320px, 1fr)' }, gap: 2 }}>
            <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 } }}>
              <Stack spacing={1.5}>
                <Typography component="h3" variant="h6">Productos</Typography>
                {!showCatalog && <Alert severity="info">El catálogo no está disponible para nuevas líneas en este contexto.</Alert>}
                {showCatalog && (
                  <>
                    {categoriesQuery.error && (
                      <Alert severity="warning">
                        {errorCode(categoriesQuery.error) === 'FORBIDDEN'
                          ? 'No tienes permiso para consultar el catálogo de esta sucursal.'
                          : 'No se pudieron cargar las categorías.'}
                      </Alert>
                    )}
                    <Stack direction="row" spacing={1} sx={{ overflowX: 'auto', pb: 0.5 }}>
                      <Button size="small" variant={!categoryId ? 'contained' : 'outlined'} onClick={() => setCategoryId('')}>Todas</Button>
                      {categories.map((category) => (
                        <Button
                          key={category.id}
                          size="small"
                          variant={categoryId === category.id ? 'contained' : 'outlined'}
                          onClick={() => setCategoryId(category.id)}
                          aria-label={category.status === 'INACTIVE' ? `${category.name}, categoría inactiva` : category.name}
                        >
                          {category.name}{category.status === 'INACTIVE' ? ' · Inactiva' : ''}
                        </Button>
                      ))}
                    </Stack>
                    {categoriesQuery.isLoading && <Skeleton variant="rounded" height={36} />}
                    {categoriesQuery.data && categoriesQuery.data.totalPages > 1 && (
                      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
                        <Button size="small" disabled={categoryPage === 0} onClick={() => setCategoryPage(categoryPage - 1)}>Categorías anteriores</Button>
                        <Typography variant="caption">{categoryPage + 1}/{categoriesQuery.data.totalPages}</Typography>
                        <Button size="small" disabled={categoryPage + 1 >= categoriesQuery.data.totalPages} onClick={() => setCategoryPage(categoryPage + 1)}>Más categorías</Button>
                      </Stack>
                    )}
                    <TextField
                      label="Buscar productos"
                      value={searchInput}
                      onChange={(event) => setSearchInput(event.target.value)}
                      fullWidth
                      size="small"

                    />
                    {productsQuery.error && (
                      <Alert severity="warning">
                        {errorCode(productsQuery.error) === 'FORBIDDEN'
                          ? 'No tienes permiso para consultar el catálogo de esta sucursal.'
                          : 'No se pudieron cargar los productos.'}
                      </Alert>
                    )}
                    {productsQuery.isLoading && <Stack spacing={1}><Skeleton variant="rounded" height={76} /><Skeleton variant="rounded" height={76} /></Stack>}
                    {productsQuery.data?.items.length === 0 && <Alert severity="info">No hay productos que coincidan con esta búsqueda.</Alert>}
                    <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))' }, gap: 1 }}>
                      {products.map((product) => (
                        <Button
                          key={product.id}
                          variant="outlined"
                          onClick={() => onOpenAdd(product)}
                          disabled={!showCatalog || pending}
                          sx={{ justifyContent: 'space-between', textAlign: 'left', minHeight: 76, px: 1.5 }}
                        >
                          <Stack sx={{ minWidth: 0, alignItems: 'flex-start' }}>
                            <Typography variant="body2" sx={{ fontWeight: 700, overflowWrap: 'anywhere' }}>{product.name}</Typography>
                            <Typography variant="caption" color="text.secondary">SKU {product.sku}</Typography>
                          </Stack>
                          <Typography variant="body2" sx={{ whiteSpace: 'nowrap', pl: 1 }}>{formatMoney(product.unitPrice, locale, currency)}</Typography>
                        </Button>
                      ))}
                    </Box>
                    {productsQuery.data && productsQuery.data.totalPages > 1 && (
                      <Stack direction="row" spacing={1} sx={{ alignItems: 'center', justifyContent: 'space-between' }}>
                        <Button disabled={productPage === 0} onClick={() => setProductPage(productPage - 1)}>Productos anteriores</Button>
                        <Typography variant="body2">{productPage + 1}/{productsQuery.data.totalPages}</Typography>
                        <Button disabled={productPage + 1 >= productsQuery.data.totalPages} onClick={() => setProductPage(productPage + 1)}>Más productos</Button>
                      </Stack>
                    )}
                  </>
                )}
              </Stack>
            </Paper>

            <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 } }}>
              <Stack spacing={1.5}>
                <Stack direction="row" sx={{ alignItems: 'center', justifyContent: 'space-between', gap: 1 }}>
                  <Box>
                    <Typography component="h3" variant="h6">Venta {shortSaleId(saleDetail.sale.id)}</Typography>
                    <Typography variant="caption" color="text.secondary">{saleDetail.sale.status === 'OPEN' ? 'Reanudando venta abierta' : saleDetail.sale.status}</Typography>
                  </Box>
                  <Chip label={`v${saleDetail.sale.version}`} size="small" variant="outlined" />
                </Stack>
                {activeItems.length === 0 && <Alert severity="info">Tu venta está vacía. Selecciona un producto para comenzar.</Alert>}
                <Stack spacing={1}>
                  {activeItems.map((item) => (
                    <Box key={item.id} sx={{ borderBottom: 1, borderColor: 'divider', pb: 1.25 }}>
                      <Stack direction="row" spacing={1} sx={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
                        <Box sx={{ minWidth: 0 }}>
                          <Typography variant="body2" sx={{ fontWeight: 700 }}>{item.quantity} × {item.productNameSnapshot}</Typography>
                          {item.modifiers.map((modifier) => (
                            <Typography key={modifier.id} component="div" variant="caption" color="text.secondary">
                              {modifier.groupNameSnapshot}: {modifier.optionNameSnapshot}
                            </Typography>
                          ))}
                          <Typography variant="caption" color="text.secondary">{formatMoney(item.effectiveUnitPrice, locale, currency)} c/u</Typography>
                        </Box>
                        <Stack sx={{ alignItems: 'flex-end', flexShrink: 0 }}>
                          <Typography variant="body2" sx={{ fontWeight: 700 }}>{formatMoney(item.lineTotal, locale, currency)}</Typography>
                          <Stack direction="row" spacing={0.25}>
                            <Button aria-label={`Editar ${item.productNameSnapshot}`} size="small" disabled={!showCatalog || pending} onClick={() => onOpenEdit(item)}>Editar</Button>
                            <Button aria-label={`Quitar ${item.productNameSnapshot}`} size="small" color="error" disabled={!showCatalog || pending} onClick={() => onRemove(item)}>Quitar</Button>
                          </Stack>
                        </Stack>
                      </Stack>
                    </Box>
                  ))}
                </Stack>
                {removedItems.length > 0 && (
                  <details>
                    <summary>Elementos retirados ({removedItems.length})</summary>
                    <Stack spacing={0.5} sx={{ pt: 1 }}>
                      {removedItems.map((item) => <Typography key={item.id} variant="body2" color="text.secondary">{item.quantity} × {item.productNameSnapshot} · Retirado</Typography>)}
                    </Stack>
                  </details>
                )}
                <Stack direction="row" sx={{ alignItems: 'baseline', justifyContent: 'space-between', borderTop: 1, borderColor: 'divider', pt: 1.5 }}>
                  <Typography variant="h6">Total</Typography>
                  <Typography variant="h5" aria-label="Total de la venta">{formatMoney(saleDetail.sale.totalAmount, locale, currency)}</Typography>
                </Stack>
                {saleDetail.sale.status === 'OPEN' && (
                  <Button
                    color="error"
                    variant="outlined"

                    onClick={onCancel}
                    disabled={pending}
                  >
                    Cancelar venta
                  </Button>
                )}
              </Stack>
            </Paper>
          </Box>
        </>
      )}
    </Stack>
  )
}

type GroupControlProps = {
  group: ModifierGroup
  selectedOptionIds: string[]
  locale: string
  currency: string
  onToggle: (id: string, checked: boolean) => void
}

function ModifierGroupControl({ group, selectedOptionIds, locale, currency, onToggle }: GroupControlProps) {
  if (group.status === 'INACTIVE' && group.minSelections === 0) return null
  const isRequiredUnavailable = group.status === 'INACTIVE' && group.minSelections > 0
  const options = group.options
  const selectedCount = options.filter((option) => selectedOptionIds.includes(option.id)).length
  const single = group.maxSelections === 1

  return (
    <FormControl component="fieldset" disabled={group.status !== 'ACTIVE'}>
      <FormLabel component="legend">{group.name} · {group.minSelections > 0 ? `Obligatorio (${group.minSelections} mínimo)` : 'Opcional'}{group.maxSelections > 1 ? ` · hasta ${group.maxSelections}` : ''}</FormLabel>
      {isRequiredUnavailable && <Typography variant="caption" color="error">Este grupo obligatorio está inactivo.</Typography>}
      {!isRequiredUnavailable && group.status === 'ACTIVE' && (
        <Typography variant="caption" color="text.secondary">Seleccionadas: {selectedCount} de {group.maxSelections}</Typography>
      )}
      {single ? (
        <RadioGroup value={selectedOptionIds.find((id) => options.some((option) => option.id === id)) ?? ''}>
          {options.map((option) => (
            <FormControlLabel
              key={option.id}
              value={option.id}
              control={<Radio checked={selectedOptionIds.includes(option.id)} onChange={(event) => onToggle(option.id, event.target.checked)} />}
              label={`${option.name}${option.status === 'INACTIVE' ? ' · No disponible' : ''} · ${formatMoney(option.priceDelta, locale, currency)}`}
              disabled={option.status !== 'ACTIVE' || group.status !== 'ACTIVE'}
            />
          ))}
        </RadioGroup>
      ) : options.map((option) => (
        <FormControlLabel
          key={option.id}
          control={<Checkbox checked={selectedOptionIds.includes(option.id)} onChange={(event) => onToggle(option.id, event.target.checked)} />}
          label={`${option.name}${option.status === 'INACTIVE' ? ' · No disponible' : ''} · ${formatMoney(option.priceDelta, locale, currency)}`}
          disabled={option.status !== 'ACTIVE' || group.status !== 'ACTIVE'
            || (!selectedOptionIds.includes(option.id) && selectedCount >= group.maxSelections)}
        />
      ))}
    </FormControl>
  )
}
