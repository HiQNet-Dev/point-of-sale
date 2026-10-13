import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'

export function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

export function inventoryItemErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION':
      return 'El artículo cambió desde que lo abriste. Se actualizará la lista.'
    case 'INVENTORY_ITEM_CODE_ALREADY_EXISTS':
      return 'Ya existe un artículo con ese código en la organización.'
    case 'INVENTORY_ITEM_NOT_FOUND':
      return 'El artículo ya no existe.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos ingresados.'
    case 'ORGANIZATION_NOT_FOUND':
      return 'La organización no está disponible.'
    default:
      if (error instanceof ApiRequestError && error.status === 404) return 'El artículo ya no existe.'
      if (error instanceof ApiRequestError && error.status === 403) {
        return 'No tienes permiso para esta operación de inventario.'
      }
      if (error instanceof ApiNetworkError) return 'No se pudo contactar al servidor.'
      return 'No se pudo completar la operación.'
  }
}
