import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, MenuItem, Stack, Table, TableBody, TableCell, TableHead, TableRow, TextField, Typography } from '@mui/material'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'
import type { ProductDto } from '../../shared/rust-local/desktop.generated'

function messageFor(error: unknown): string {
  if (error instanceof DesktopCommandError) {
    switch (error.code) {
      case 'FORBIDDEN':
        return 'La cuenta no tiene permiso para esta operación.'
      case 'CONFLICT':
        return 'El código o SKU ya existe, o la versión cambió. Vuelve a cargar e inténtalo de nuevo.'
      case 'NOT_FOUND':
        return 'El elemento ya no existe en esta organización.'
      case 'VALIDATION_ERROR':
        return 'Revisa los datos: el precio debe ser decimal exacto (máximo 4 decimales).'
      case 'UNAUTHORIZED':
        return 'La sesión ya no es válida.'
      default:
        return 'No se pudo completar la operación.'
    }
  }
  return 'No se pudo completar la operación.'
}

/**
 * Catálogo real detrás de la sesión.
 *
 * La UI solo insinúa permisos; la decisión y el aislamiento por organización se
 * resuelven en SQLite. Los precios son texto decimal exacto de extremo a extremo.
 */
export function CatalogWorkspace({ permissions }: { permissions: string[] }) {
  const queryClient = useQueryClient()
  const canCreateProduct = permissions.includes('product.create')
  const canUpdateProduct = permissions.includes('product.update')
  const canCreateCategory = permissions.includes('category.create')

  const categories = useQuery({ queryKey: ['rust-desktop', 'categories'], queryFn: () => desktopService.catalogListCategories(), retry: false })
  const products = useQuery({ queryKey: ['rust-desktop', 'products'], queryFn: () => desktopService.catalogListProducts(), retry: false })

  const [categoryCode, setCategoryCode] = useState('')
  const [categoryName, setCategoryName] = useState('')
  const [sku, setSku] = useState('')
  const [productName, setProductName] = useState('')
  const [price, setPrice] = useState('')
  const [categoryId, setCategoryId] = useState('')
  const [editing, setEditing] = useState<ProductDto | null>(null)
  const [editName, setEditName] = useState('')
  const [editPrice, setEditPrice] = useState('')
  const [editStatus, setEditStatus] = useState('ACTIVE')

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'categories'] })
    void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'products'] })
  }

  const createCategory = useMutation({
    mutationFn: () => desktopService.catalogCreateCategory(categoryCode, categoryName),
    onSuccess: () => {
      setCategoryCode('')
      setCategoryName('')
      refresh()
    },
  })
  const createProduct = useMutation({
    mutationFn: () =>
      desktopService.catalogCreateProduct({
        sku,
        name: productName,
        unitPrice: price,
        categoryId: categoryId || null,
        barcode: null,
        description: null,
      }),
    onSuccess: () => {
      setSku('')
      setProductName('')
      setPrice('')
      refresh()
    },
  })
  const updateProduct = useMutation({
    mutationFn: () =>
      desktopService.catalogUpdateProduct({
        productId: editing?.id ?? '',
        expectedVersion: editing?.version ?? '0',
        name: editName,
        unitPrice: editPrice,
        status: editStatus,
      }),
    onSuccess: () => {
      setEditing(null)
      refresh()
    },
  })

  const openEdit = (product: ProductDto) => {
    setEditing(product)
    setEditName(product.name)
    setEditPrice(product.unitPrice)
    setEditStatus('ACTIVE')
  }

  return (
    <Stack spacing={3} component="section" aria-labelledby="catalog-heading">
      <Typography id="catalog-heading" variant="h5" component="h2">Catálogo</Typography>

      <Box component="section" aria-labelledby="categories-heading">
        <Typography id="categories-heading" variant="h6" component="h3">Categorías</Typography>
        {categories.isPending ? <Typography role="status">Cargando categorías…</Typography> : null}
        {categories.isError ? <Alert severity="error" role="alert">{messageFor(categories.error)}</Alert> : null}
        {categories.data ? (
          <Table size="small" aria-label="Categorías">
            <TableHead><TableRow><TableCell>Código</TableCell><TableCell>Nombre</TableCell></TableRow></TableHead>
            <TableBody>
              {categories.data.map((category) => (
                <TableRow key={category.id}><TableCell>{category.code}</TableCell><TableCell>{category.name}</TableCell></TableRow>
              ))}
            </TableBody>
          </Table>
        ) : null}
        {canCreateCategory ? (
          <Box
            component="form"
            aria-label="Nueva categoría"
            onSubmit={(event) => {
              event.preventDefault()
              if (!createCategory.isPending) createCategory.mutate()
            }}
            sx={{ mt: 2 }}
          >
            <Stack direction="row" spacing={1}>
              <TextField label="Código" value={categoryCode} onChange={(event) => setCategoryCode(event.target.value)} required size="small" />
              <TextField label="Nombre" value={categoryName} onChange={(event) => setCategoryName(event.target.value)} required size="small" />
              <Button type="submit" variant="outlined" disabled={createCategory.isPending}>Añadir categoría</Button>
            </Stack>
          </Box>
        ) : null}
        {createCategory.isError ? <Alert severity="error" role="alert" sx={{ mt: 1 }}>{messageFor(createCategory.error)}</Alert> : null}
      </Box>

      <Box component="section" aria-labelledby="products-heading">
        <Typography id="products-heading" variant="h6" component="h3">Productos</Typography>
        {products.isPending ? <Typography role="status">Cargando productos…</Typography> : null}
        {products.isError ? <Alert severity="error" role="alert">{messageFor(products.error)}</Alert> : null}
        {products.data ? (
          <Table size="small" aria-label="Productos">
            <TableHead><TableRow><TableCell>SKU</TableCell><TableCell>Nombre</TableCell><TableCell>Precio</TableCell><TableCell>Versión</TableCell><TableCell /></TableRow></TableHead>
            <TableBody>
              {products.data.map((product) => (
                <TableRow key={product.id}>
                  <TableCell>{product.sku}</TableCell>
                  <TableCell>{product.name}</TableCell>
                  <TableCell>{product.unitPrice}</TableCell>
                  <TableCell>{product.version}</TableCell>
                  <TableCell>
                    {canUpdateProduct ? (
                      <Button size="small" onClick={() => openEdit(product)}>Editar</Button>
                    ) : null}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : null}
        {canCreateProduct ? (
          <Box
            component="form"
            aria-label="Nuevo producto"
            onSubmit={(event) => {
              event.preventDefault()
              if (!createProduct.isPending) createProduct.mutate()
            }}
            sx={{ mt: 2 }}
          >
            <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap" }}>
              <TextField label="SKU" value={sku} onChange={(event) => setSku(event.target.value)} required size="small" />
              <TextField label="Nombre" value={productName} onChange={(event) => setProductName(event.target.value)} required size="small" />
              <TextField label="Precio (4 decimales)" value={price} onChange={(event) => setPrice(event.target.value)} required size="small" placeholder="0.0000" />
              <TextField select label="Categoría" value={categoryId} onChange={(event) => setCategoryId(event.target.value)} size="small" sx={{ minWidth: 180 }}>
                <MenuItem value="">Sin categoría</MenuItem>
                {(categories.data ?? []).map((category) => (
                  <MenuItem key={category.id} value={category.id}>{category.name}</MenuItem>
                ))}
              </TextField>
              <Button type="submit" variant="contained" disabled={createProduct.isPending}>Añadir producto</Button>
            </Stack>
          </Box>
        ) : null}
        {createProduct.isError ? <Alert severity="error" role="alert" sx={{ mt: 1 }}>{messageFor(createProduct.error)}</Alert> : null}
      </Box>

      {editing ? (
        <Box component="section" aria-labelledby="edit-product-heading">
          <Typography id="edit-product-heading" variant="h6" component="h3">Editar {editing.sku}</Typography>
          <Box
            component="form"
            aria-label="Editar producto"
            onSubmit={(event) => {
              event.preventDefault()
              if (!updateProduct.isPending) updateProduct.mutate()
            }}
            sx={{ mt: 1 }}
          >
            <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap" }}>
              <TextField label="Nombre" value={editName} onChange={(event) => setEditName(event.target.value)} size="small" />
              <TextField label="Precio (4 decimales)" value={editPrice} onChange={(event) => setEditPrice(event.target.value)} size="small" />
              <TextField select label="Estado" value={editStatus} onChange={(event) => setEditStatus(event.target.value)} size="small">
                <MenuItem value="ACTIVE">Activo</MenuItem>
                <MenuItem value="INACTIVE">Inactivo</MenuItem>
              </TextField>
              <Button type="submit" variant="contained" disabled={updateProduct.isPending}>Guardar</Button>
              <Button onClick={() => setEditing(null)}>Cancelar</Button>
            </Stack>
          </Box>
          {updateProduct.isError ? <Alert severity="error" role="alert" sx={{ mt: 1 }}>{messageFor(updateProduct.error)}</Alert> : null}
        </Box>
      ) : null}
    </Stack>
  )
}
