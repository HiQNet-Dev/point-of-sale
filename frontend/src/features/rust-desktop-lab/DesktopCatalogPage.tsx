import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, Container, Stack, Table, TableBody, TableCell, TableHead, TableRow, TextField, Typography } from '@mui/material'
import { desktopService } from '../../shared/rust-local/desktop-client'

/** Pantalla aislada: no sustituye login ni administración del backend Java. */
export function DesktopCatalogPage() {
  const [offset, setOffset] = useState(0)
  const [sku, setSku] = useState('')
  const [name, setName] = useState('')
  const [price, setPrice] = useState('')
  const queryClient = useQueryClient()
  const branches = useQuery({ queryKey: ['rust-lab', 'branches'], queryFn: desktopService.listBranches, retry: false })
  const categories = useQuery({ queryKey: ['rust-lab', 'categories', offset], queryFn: () => desktopService.listCategories(offset), retry: false })
  const products = useQuery({ queryKey: ['rust-lab', 'products', offset], queryFn: () => desktopService.listProducts(offset), retry: false })
  const createProduct = useMutation({
    mutationFn: () => desktopService.createProduct({ sku: sku.trim(), name: name.trim(), unitPrice: price.trim() }),
    onSuccess: () => {
      setSku('')
      setName('')
      setPrice('')
      void queryClient.invalidateQueries({ queryKey: ['rust-lab', 'products'] })
    },
  })
  const error = branches.error ?? categories.error ?? products.error
  return <Container component="main" maxWidth="lg" sx={{ py: 4 }}>
    <Stack spacing={3}>
      <Typography variant="h4" component="h1">Catálogo Rust · laboratorio IPC</Typography>
      <Alert severity="warning">SQLite temporal y sesión sintética. No productivo. No utiliza Java ni bases de clientes.</Alert>
      {error && <Alert severity="error" role="alert">No se pudo consultar el backend: {error.message}</Alert>}
      {(branches.isPending || categories.isPending || products.isPending) && <Typography role="status">Consultando Rust por IPC…</Typography>}
      <Box component="section" aria-labelledby="branches-heading">
        <Typography id="branches-heading" variant="h6" component="h2">Sucursales</Typography>
        <ul>{branches.data?.map(branch => <li key={branch.id}>{branch.name} · {branch.code} · {branch.status}</li>)}</ul>
      </Box>
      <Box component="section" aria-labelledby="categories-heading">
        <Typography id="categories-heading" variant="h6" component="h2">Categorías</Typography>
        <ul>{categories.data?.map(category => <li key={category.id}>{category.name} · {category.code}</li>)}</ul>
      </Box>
      <Box component="section" aria-labelledby="create-product-heading">
        <Typography id="create-product-heading" variant="h6" component="h2">Crear producto</Typography>
        <Stack component="form" direction={{ xs: 'column', sm: 'row' }} spacing={2} onSubmit={event => { event.preventDefault(); createProduct.mutate() }}>
          <TextField label="SKU" value={sku} onChange={event => setSku(event.target.value)} size="small" required />
          <TextField label="Nombre" value={name} onChange={event => setName(event.target.value)} size="small" required />
          <TextField label="Precio (escala 4)" value={price} onChange={event => setPrice(event.target.value)} size="small" required placeholder="0.0000" />
          <Button type="submit" variant="contained" disabled={createProduct.isPending}>Crear producto</Button>
        </Stack>
        {createProduct.isError && <Alert severity="error" role="alert" sx={{ mt: 2 }}>No se pudo crear el producto: {createProduct.error.message}</Alert>}
        {createProduct.isSuccess && <Alert severity="success" sx={{ mt: 2 }}>Producto creado: {createProduct.data.sku}</Alert>}
      </Box>
      <Table aria-label="Productos consultados por IPC">
        <TableHead><TableRow><TableCell>SKU</TableCell><TableCell>Producto</TableCell><TableCell align="right">Precio exacto (escala 4)</TableCell><TableCell>Versión</TableCell></TableRow></TableHead>
        <TableBody>{products.data?.map(product => <TableRow key={product.id}>
          <TableCell>{product.sku}</TableCell><TableCell>{product.name}</TableCell>
          <TableCell align="right">{product.unitPrice}</TableCell><TableCell>{product.version}</TableCell>
        </TableRow>)}</TableBody>
      </Table>
      <Stack direction="row" spacing={2}>
        <Button disabled={offset === 0} onClick={() => setOffset(value => Math.max(0, value - 20))}>Anterior</Button>
        <Button disabled={products.isFetching || (products.data?.length ?? 0) < 20} onClick={() => setOffset(value => value + 20)}>Siguiente</Button>
        <Button onClick={() => { void branches.refetch(); void categories.refetch(); void products.refetch() }}>Actualizar</Button>
      </Stack>
    </Stack>
  </Container>
}
