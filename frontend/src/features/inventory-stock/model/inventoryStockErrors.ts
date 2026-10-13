import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'

export function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

export function inventoryStockErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION':
      return 'Las existencias cambiaron desde que se cargaron. Se actualizarán los datos.'
    case 'INVENTORY_OPENING_ALREADY_EXISTS':
      return 'Ya existe una apertura de existencias para este artículo en esta sucursal.'
    case 'INVENTORY_HISTORY_ALREADY_EXISTS':
      return 'La apertura solo se permite antes de cualquier movimiento de existencias.'
    case 'INVENTORY_ITEM_INACTIVE':
      return 'El artículo está inactivo; no se pueden registrar movimientos.'
    case 'BRANCH_INACTIVE':
      return 'La sucursal está inactiva; no se pueden registrar movimientos.'
    case 'INVALID_INVENTORY_QUANTITY':
      return 'La cantidad no es válida.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos ingresados.'
    case 'BRANCH_NOT_FOUND':
      return 'La sucursal no está disponible.'
    case 'INVENTORY_ITEM_NOT_FOUND':
      return 'El artículo ya no existe.'
    default:
      if (error instanceof ApiRequestError && error.status === 403) {
        return 'No tienes permiso para esta operación de existencias.'
      }
      if (error instanceof ApiRequestError && error.status === 404) return 'El recurso ya no existe.'
      if (error instanceof ApiNetworkError) return 'No se pudo contactar al servidor.'
      return 'No se pudo completar la operación.'
  }
}
