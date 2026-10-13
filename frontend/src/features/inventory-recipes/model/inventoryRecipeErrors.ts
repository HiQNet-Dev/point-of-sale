import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'

export function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

/**
 * Only the recipe GET treats absence as an editable empty state, and only for
 * its own error code. Other 404s must stay errors.
 */
export function isRecipeAbsent(error: unknown): boolean {
  return errorCode(error) === 'INVENTORY_RECIPE_NOT_FOUND'
}

export function inventoryRecipeErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION':
      return 'La receta cambió mientras la editabas. Actualizamos la información; revisa los cambios e intenta de nuevo.'
    case 'INVENTORY_CONFIGURATION_UNAVAILABLE':
      return 'Uno de los artículos seleccionados ya no está disponible para nuevas configuraciones. Actualiza la receta e intenta de nuevo.'
    case 'INVENTORY_ITEM_INACTIVE':
      return 'Los componentes nuevos deben referenciar artículos activos. Retira los artículos inactivos antes de activar la receta.'
    case 'INVALID_INVENTORY_QUANTITY':
      return 'La cantidad no es válida.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos ingresados.'
    case 'INVENTORY_ITEM_NOT_FOUND':
      return 'Uno de los artículos ya no existe.'
    case 'PRODUCT_NOT_FOUND':
      return 'El producto ya no está disponible.'
    case 'ORGANIZATION_NOT_FOUND':
      return 'La organización no está disponible.'
    default:
      if (error instanceof ApiRequestError && error.status === 403) {
        return 'No tienes permiso para administrar recetas de inventario.'
      }
      if (error instanceof ApiRequestError && error.status === 404) return 'El recurso ya no existe.'
      if (error instanceof ApiNetworkError) return 'No se pudo contactar al servidor.'
      return 'No se pudo completar la operación.'
  }
}
