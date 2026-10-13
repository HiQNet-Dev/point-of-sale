import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'

export function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

/** Only the effects GET treats absence as an editable empty state. */
export function isEffectsAbsent(error: unknown): boolean {
  return errorCode(error) === 'INVENTORY_MODIFIER_CONFIG_NOT_FOUND'
}

export function inventoryModifierEffectsErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION':
      return 'Los efectos cambiaron mientras los editabas. Actualizamos la información; revisa los cambios e intenta de nuevo.'
    case 'INVENTORY_CONFIGURATION_UNAVAILABLE':
      return 'Uno de los artículos seleccionados ya no está disponible para nuevas configuraciones. Actualiza los efectos e intenta de nuevo.'
    case 'INVENTORY_ITEM_INACTIVE':
      return 'Los efectos nuevos deben referenciar artículos activos. Retira los artículos inactivos antes de guardar.'
    case 'INVALID_INVENTORY_QUANTITY':
      return 'El valor del efecto no es válido.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos ingresados.'
    case 'INVENTORY_ITEM_NOT_FOUND':
      return 'Uno de los artículos ya no existe.'
    case 'MODIFIER_OPTION_NOT_FOUND':
      return 'La opción de modificador ya no está disponible.'
    case 'ORGANIZATION_NOT_FOUND':
      return 'La organización no está disponible.'
    default:
      if (error instanceof ApiRequestError && error.status === 403) {
        return 'No tienes permiso para administrar efectos de inventario.'
      }
      if (error instanceof ApiRequestError && error.status === 404) return 'El recurso ya no existe.'
      if (error instanceof ApiNetworkError) return 'No se pudo contactar al servidor.'
      return 'No se pudo completar la operación.'
  }
}
