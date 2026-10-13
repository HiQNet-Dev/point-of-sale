import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import CircularProgress from '@mui/material/CircularProgress'
import FormControl from '@mui/material/FormControl'
import InputLabel from '@mui/material/InputLabel'
import MenuItem from '@mui/material/MenuItem'
import Paper from '@mui/material/Paper'
import Select from '@mui/material/Select'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { QUANTITY_UNIT_SYMBOLS } from '../../inventory-items/api/inventoryItemsApi'
import { InventoryItemPicker } from '../../inventory-items/components/InventoryItemPicker'
import type { SelectedInventoryItem } from '../../inventory-items/components/InventoryItemPicker'
import { formatQuantity } from '../../inventory-stock/model/inventoryQuantity'
import {
  getProductRecipe,
  RECIPE_STATUSES,
  RECIPE_STATUS_LABELS,
  replaceProductRecipe,
} from '../api/inventoryRecipesApi'
import type { Product, ProductRecipe, RecipeStatus } from '../api/inventoryRecipesApi'
import { inventoryRecipesKeys } from '../model/inventoryRecipesKeys'
import { errorCode, inventoryRecipeErrorMessage, isRecipeAbsent } from '../model/inventoryRecipeErrors'
import { validateRecipeQuantity } from '../model/inventoryRecipeValidation'

interface EditorRow {
  key: string
  item: SelectedInventoryItem | null
  quantity: string
}

export interface RecipeEditorProps {
  organizationId: string
  product: Product
  dirtyRef?: RefObject<boolean>
}

function componentItem(component: ProductRecipe['components'][number]): SelectedInventoryItem {
  return {
    id: component.inventoryItemId,
    code: component.code,
    name: component.name,
    baseUnit: component.baseUnit,
    fractionalUnitAllowed: component.fractionalUnitAllowed,
    status: component.status,
  }
}

export function RecipeEditor({ organizationId, product, dirtyRef }: RecipeEditorProps) {
  const recipeQuery = useQuery({
    queryKey: inventoryRecipesKeys.recipe(organizationId, product.id),
    queryFn: () => getProductRecipe(organizationId, product.id),
    enabled: Boolean(organizationId),
    retry: false,
    refetchOnWindowFocus: false,
  })

  const absent = recipeQuery.isError && isRecipeAbsent(recipeQuery.error)
  const recipe = recipeQuery.data ?? null

  if (recipeQuery.isPending) {
    return (
      <Box sx={{ display: 'grid', placeItems: 'center', py: 4 }}>
        <CircularProgress size={28} aria-label="Cargando receta" />
      </Box>
    )
  }

  if (recipeQuery.isError && !absent) {
    return <Alert severity="error" role="alert">{inventoryRecipeErrorMessage(recipeQuery.error)}</Alert>
  }

  return (
    <RecipeForm
      key={recipe ? `v:${recipe.version}` : 'absent'}
      organizationId={organizationId}
      product={product}
      recipe={recipe}
      dirtyRef={dirtyRef}
    />
  )
}

interface RecipeFormProps {
  organizationId: string
  product: Product
  recipe: ProductRecipe | null
  dirtyRef?: RefObject<boolean>
}

function RecipeForm({ organizationId, product, recipe, dirtyRef }: RecipeFormProps) {
  const queryClient = useQueryClient()
  const keyRef = useRef(0)

  const [status, setStatus] = useState<RecipeStatus>(recipe?.status ?? 'ACTIVE')
  const [rows, setRows] = useState<EditorRow[]>(() =>
    recipe
      ? recipe.components.map((component) => ({
          key: component.inventoryItemId,
          item: componentItem(component),
          quantity: formatQuantity(component.quantity),
        }))
      : [],
  )
  const [banner, setBanner] = useState('')
  const [formError, setFormError] = useState('')

  const baseline = recipe
    ? JSON.stringify({
        status: recipe.status,
        components: recipe.components
          .map((component) => [component.inventoryItemId, formatQuantity(component.quantity)])
          .sort(),
      })
    : JSON.stringify({ status: 'ACTIVE', components: [] })
  const current = JSON.stringify({
    status,
    components: rows.map((row) => [row.item?.id ?? '', row.quantity.trim()]).sort(),
  })
  const dirty = baseline !== current

  useEffect(() => {
    if (dirtyRef) dirtyRef.current = dirty
  }, [dirty, dirtyRef])

  const saveMutation = useMutation({
    mutationFn: () =>
      replaceProductRecipe(organizationId, product.id, {
        expectedVersion: recipe?.version ?? 0,
        status,
        components: rows.map((row) => ({
          inventoryItemId: row.item?.id ?? '',
          quantity: row.quantity.trim(),
        })),
      }),
    onSuccess: (saved) => {
      queryClient.setQueryData(inventoryRecipesKeys.recipe(organizationId, product.id), saved)
      setBanner('Receta guardada.')
      void queryClient.invalidateQueries({
        queryKey: inventoryRecipesKeys.recipe(organizationId, product.id),
      })
    },
    onError: (error) => {
      setBanner(inventoryRecipeErrorMessage(error))
      if (errorCode(error) === 'CONCURRENT_MODIFICATION') {
        void queryClient.invalidateQueries({
          queryKey: inventoryRecipesKeys.recipe(organizationId, product.id),
        })
      }
    },
  })

  function addRow() {
    setRows((previous) => [...previous, { key: `new-${keyRef.current++}`, item: null, quantity: '' }])
  }

  function removeRow(key: string) {
    setRows((previous) => previous.filter((row) => row.key !== key))
  }

  function setRowItem(key: string, item: SelectedInventoryItem | null) {
    setRows((previous) => previous.map((row) => (row.key === key ? { ...row, item } : row)))
  }

  function setRowQuantity(key: string, quantity: string) {
    setRows((previous) => previous.map((row) => (row.key === key ? { ...row, quantity } : row)))
  }

  function handleSubmit() {
    for (const row of rows) {
      if (!row.item) {
        setFormError('Selecciona un artículo en cada componente.')
        return
      }
      const message = validateRecipeQuantity(row.quantity, row.item)
      if (message) {
        setFormError(message)
        return
      }
    }
    const ids = rows.map((row) => row.item?.id)
    if (new Set(ids).size !== ids.length) {
      setFormError('No puedes repetir el mismo artículo en la receta.')
      return
    }
    if (status === 'ACTIVE' && rows.length === 0) {
      setFormError('Una receta activa debe tener al menos un componente.')
      return
    }
    setFormError('')
    saveMutation.mutate()
  }

  function handleCancel() {
    setStatus(recipe?.status ?? 'ACTIVE')
    setRows(
      recipe
        ? recipe.components.map((component) => ({
            key: component.inventoryItemId,
            item: componentItem(component),
            quantity: formatQuantity(component.quantity),
          }))
        : [],
    )
    setFormError('')
    setBanner('')
  }

  return (
    <Paper variant="outlined" sx={{ p: 2 }}>
      <Stack spacing={2}>
        <Box>
          <Typography component="h3" variant="h6">{product.name}</Typography>
          <Typography variant="body2" color="text.secondary">
            {product.sku}
            {recipe ? '' : ' · Sin receta configurada'}
          </Typography>
        </Box>

        {banner && <Alert severity="info" role="status" onClose={() => setBanner('')}>{banner}</Alert>}
        {formError && <Alert severity="error" role="alert">{formError}</Alert>}

        <Typography variant="body2" color="text.secondary">
          La receta define el consumo base del producto. Cada artículo aparece una sola vez con una cantidad exacta.
        </Typography>

        <FormControl sx={{ minWidth: 200, maxWidth: 260 }}>
          <InputLabel id="recipe-status-label">Estado de la receta</InputLabel>
          <Select
            labelId="recipe-status-label"
            label="Estado de la receta"
            value={status}
            onChange={(event) => setStatus(event.target.value as RecipeStatus)}
          >
            {RECIPE_STATUSES.map((value) => (
              <MenuItem key={value} value={value}>{RECIPE_STATUS_LABELS[value]}</MenuItem>
            ))}
          </Select>
        </FormControl>

        <Stack spacing={2}>
          {rows.map((row) => {
            const otherIds = rows.filter((other) => other.key !== row.key && other.item).map((other) => other.item!.id)
            const quantityError = row.quantity.trim() ? validateRecipeQuantity(row.quantity, row.item) : null
            return (
              <Stack
                key={row.key}
                direction={{ xs: 'column', sm: 'row' }}
                spacing={2}
                sx={{ alignItems: { xs: 'stretch', sm: 'flex-start' } }}
              >
                <Box sx={{ flexGrow: 1, minWidth: 0 }}>
                  <InventoryItemPicker
                    organizationId={organizationId}
                    value={row.item}
                    onChange={(item) => setRowItem(row.key, item)}
                    disabledItemIds={otherIds}
                    error={!row.item && Boolean(formError)}
                  />
                </Box>
                <TextField
                  label="Cantidad"
                  value={row.quantity}
                  onChange={(event) => setRowQuantity(row.key, event.target.value)}
                  error={Boolean(quantityError)}
                  helperText={quantityError ?? (row.item ? QUANTITY_UNIT_SYMBOLS[row.item.baseUnit] : ' ')}
                  sx={{ width: { xs: '100%', sm: 180 } }}
                />
                <Button
                  onClick={() => removeRow(row.key)}
                  aria-label={`Quitar componente ${row.item ? row.item.name : ''}`.trim()}
                >
                  Quitar
                </Button>
              </Stack>
            )
          })}
          {rows.length === 0 && (
            <Typography variant="body2" color="text.secondary">
              Este producto aún no tiene una receta de inventario.
            </Typography>
          )}
        </Stack>

        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1}>
          <Button variant="outlined" onClick={addRow}>Añadir componente</Button>
          <Box sx={{ flexGrow: 1 }} />
          <Button onClick={handleCancel} disabled={saveMutation.isPending}>Cancelar</Button>
          <Button variant="contained" onClick={handleSubmit} disabled={saveMutation.isPending}>
            Guardar receta
          </Button>
        </Stack>
      </Stack>
    </Paper>
  )
}
