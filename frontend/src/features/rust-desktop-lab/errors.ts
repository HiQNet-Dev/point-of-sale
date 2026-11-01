import { DesktopCommandError } from '../../shared/rust-local/desktop-client'

/** Código público del adaptador IPC, o `null` si el fallo no vino de Rust. */
export function codeOf(error: unknown): string | null {
  return error instanceof DesktopCommandError ? error.code : null
}

/**
 * Mensaje comprensible por operación y código.
 *
 * El backend no expone detalles internos; la interfaz traduce el código estable
 * a una explicación útil sin inventar causas.
 */
export function describeError(error: unknown, operation: 'read' | 'write' | 'conflict'): string {
  switch (codeOf(error)) {
    case 'UNAUTHORIZED':
      return 'La sesión ya no es válida. Vuelve a iniciar sesión.'
    case 'FORBIDDEN':
      return 'La cuenta no tiene permiso para esta operación.'
    case 'CONFLICT':
      return operation === 'conflict'
        ? 'Otra sesión cambió esta configuración. Recarga para ver los valores vigentes.'
        : 'El cambio entra en conflicto con el estado actual. Recarga e inténtalo de nuevo.'
    case 'NOT_FOUND':
      return 'El recurso ya no existe en esta instalación.'
    case 'VALIDATION_ERROR':
      return 'Revisa los datos: algún valor no cumple el formato admitido.'
    case 'OPERATION_NOT_AVAILABLE':
      return 'Esta operación no está disponible en el modo actual de la aplicación.'
    default:
      return operation === 'read'
        ? 'No se pudo consultar la configuración.'
        : 'No se pudo guardar el cambio.'
  }
}
