import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'

export function errorCode(error: unknown): string | undefined {
  return error instanceof ApiRequestError ? error.code : undefined
}

export function inventorySettingsErrorMessage(error: unknown): string {
  switch (errorCode(error)) {
    case 'CONCURRENT_MODIFICATION':
      return 'La configuración de la organización cambió mientras la editabas. Actualizamos los datos; revisa la política e intenta nuevamente.'
    case 'ORGANIZATION_NOT_FOUND':
      return 'La organización no está disponible.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos ingresados.'
    default:
      if (error instanceof ApiRequestError && error.status === 403) {
        return 'No tienes permiso para esta configuración de inventario.'
      }
      if (error instanceof ApiRequestError && error.status === 404) {
        return 'La organización no está disponible.'
      }
      if (error instanceof ApiNetworkError) return 'No se pudo contactar al servidor.'
      return 'No se pudo completar la operación.'
  }
}
