import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import CircularProgress from '@mui/material/CircularProgress'
import FormControl from '@mui/material/FormControl'
import InputLabel from '@mui/material/InputLabel'
import MenuItem from '@mui/material/MenuItem'
import Paper from '@mui/material/Paper'
import Select from '@mui/material/Select'
import Stack from '@mui/material/Stack'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useAuthSession } from '../../auth/model/useAuthSession'
import {
  listCategories,
  listProducts,
  PRODUCTS_PAGE_SIZE,
  PRODUCT_STATUSES,
  PRODUCT_STATUS_LABELS,
} from '../api/inventoryRecipesApi'
import type { Product, ProductListQuery, ProductStatus } from '../api/inventoryRecipesApi'
import { inventoryRecipesKeys } from '../model/inventoryRecipesKeys'
import { inventoryRecipeErrorMessage } from '../model/inventoryRecipeErrors'
import { RecipeEditor } from '../components/RecipeEditor'

const NO_CATEGORY = '__none__'

export function RecipesPage() {
  const { me } = useAuthSession()
  const organizationId = me?.organizationId ?? ''

  const [qInput, setQInput] = useState('')
  const [q, setQ] = useState('')
  const [status, setStatus] = useState<ProductStatus | ''>('')
  const [categoryId, setCategoryId] = useState('')
  const [page, setPage] = useState(0)
  const [selected, setSelected] = useState<Product | null>(null)
  const dirtyRef = useRef(false)

  const query: ProductListQuery = {
    page,
    size: PRODUCTS_PAGE_SIZE,
    q,
    status: status || undefined,
    categoryId: categoryId && categoryId !== NO_CATEGORY ? categoryId : undefined,
  }

  const productsQuery = useQuery({
    queryKey: inventoryRecipesKeys.products(organizationId, query),
    queryFn: () => listProducts(organizationId, query),
    enabled: Boolean(organizationId),
    placeholderData: (previous) => previous,
  })

  const categoriesQuery = useQuery({
    queryKey: inventoryRecipesKeys.categories(organizationId),
    queryFn: () => listCategories(organizationId),
    enabled: Boolean(organizationId),
  })

  const categoryNames = new Map((categoriesQuery.data?.items ?? []).map((category) => [category.id, category.name]))
  const products = productsQuery.data?.items ?? []
  const totalPages = productsQuery.data?.totalPages ?? 0
  const totalElements = productsQuery.data?.totalElements ?? 0

  function selectProduct(product: Product) {
    if (product.id === selected?.id) return
    if (dirtyRef.current && !window.confirm('Tienes cambios sin guardar en la receta. ¿Deseas descartarlos?')) return
    dirtyRef.current = false
    setSelected(product)
  }

  return (
    <Stack spacing={2}>
      <Box>
        <Typography component="h2" variant="h5">Recetas de inventario</Typography>
        <Typography variant="body2" color="text.secondary">
          La receta pertenece a la organización y define el consumo base de cada producto. No depende de la sucursal.
        </Typography>
      </Box>

      <Stack direction={{ xs: 'column', md: 'row' }} spacing={2} sx={{ alignItems: 'flex-start' }}>
        <Stack spacing={2} sx={{ flex: 1, minWidth: 0, width: '100%' }}>
          <Paper variant="outlined" sx={{ p: 2 }}>
            <Stack
              direction={{ xs: 'column', md: 'row' }}
              spacing={2}
              component="form"
              onSubmit={(event) => {
                event.preventDefault()
                setQ(qInput)
                setPage(0)
              }}
            >
              <TextField
                label="Buscar producto"
                value={qInput}
                onChange={(event) => setQInput(event.target.value)}
                placeholder="Nombre o SKU"
                sx={{ flexGrow: 1 }}
              />
              <FormControl sx={{ minWidth: 160 }}>
                <InputLabel id="recipe-product-status-label">Estado</InputLabel>
                <Select
                  labelId="recipe-product-status-label"
                  label="Estado"
                  value={status}
                  onChange={(event) => {
                    setStatus(event.target.value as ProductStatus | '')
                    setPage(0)
                  }}
                >
                  <MenuItem value="">Todos</MenuItem>
                  {PRODUCT_STATUSES.map((value) => (
                    <MenuItem key={value} value={value}>{PRODUCT_STATUS_LABELS[value]}</MenuItem>
                  ))}
                </Select>
              </FormControl>
              <FormControl sx={{ minWidth: 180 }}>
                <InputLabel id="recipe-product-category-label">Categoría</InputLabel>
                <Select
                  labelId="recipe-product-category-label"
                  label="Categoría"
                  value={categoryId}
                  onChange={(event) => {
                    setCategoryId(event.target.value)
                    setPage(0)
                  }}
                >
                  <MenuItem value="">Todas</MenuItem>
                  {(categoriesQuery.data?.items ?? []).map((category) => (
                    <MenuItem key={category.id} value={category.id}>{category.name}</MenuItem>
                  ))}
                </Select>
              </FormControl>
              <Button type="submit" variant="outlined">Buscar</Button>
            </Stack>
          </Paper>

          {productsQuery.isError && (
            <Alert severity="error" role="alert">{inventoryRecipeErrorMessage(productsQuery.error)}</Alert>
          )}

          {productsQuery.isPending && (
            <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
              <CircularProgress size={28} aria-label="Cargando productos" />
            </Box>
          )}

          {productsQuery.isSuccess && (
            <TableContainer component={Paper} variant="outlined">
              <Table size="small" aria-label="Productos para recetas">
                <TableHead>
                  <TableRow>
                    <TableCell>Producto</TableCell>
                    <TableCell>SKU</TableCell>
                    <TableCell>Categoría</TableCell>
                    <TableCell>Estado</TableCell>
                    <TableCell align="right">Acciones</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {products.map((product) => (
                    <TableRow key={product.id} selected={product.id === selected?.id}>
                      <TableCell>{product.name}</TableCell>
                      <TableCell>{product.sku}</TableCell>
                      <TableCell>
                        {product.categoryId ? categoryNames.get(product.categoryId) ?? '—' : '—'}
                      </TableCell>
                      <TableCell>
                        <Chip
                          size="small"
                          label={PRODUCT_STATUS_LABELS[product.status]}
                          color={product.status === 'ACTIVE' ? 'success' : 'default'}
                        />
                      </TableCell>
                      <TableCell align="right">
                        <Button size="small" onClick={() => selectProduct(product)}>Configurar receta</Button>
                      </TableCell>
                    </TableRow>
                  ))}
                  {products.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={5}>
                        <Typography variant="body2" color="text.secondary">No hay productos disponibles.</Typography>
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </TableContainer>
          )}

          {productsQuery.isSuccess && totalElements > 0 && (
            <Stack direction="row" spacing={2} sx={{ alignItems: 'center', justifyContent: 'flex-end' }}>
              <Typography variant="body2" color="text.secondary">
                Página {page + 1} de {Math.max(totalPages, 1)}
              </Typography>
              <Button size="small" disabled={page <= 0} onClick={() => setPage((value) => Math.max(value - 1, 0))}>
                Anterior
              </Button>
              <Button size="small" disabled={page + 1 >= totalPages} onClick={() => setPage((value) => value + 1)}>
                Siguiente
              </Button>
            </Stack>
          )}
        </Stack>

        <Box sx={{ flex: 1, minWidth: 0, width: '100%' }}>
          {selected ? (
            <RecipeEditor
              key={selected.id}
              organizationId={organizationId}
              product={selected}
              dirtyRef={dirtyRef}
            />
          ) : (
            <Alert severity="info" role="status">
              Selecciona un producto para ver o configurar su receta de inventario.
            </Alert>
          )}
        </Box>
      </Stack>
    </Stack>
  )
}
