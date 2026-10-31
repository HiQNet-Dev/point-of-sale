import { commands, type CreateProductRequest_Deserialize, type IpcError } from './desktop.generated'

/** Errores públicos del adaptador IPC, sin diagnósticos de almacenamiento. */
export class DesktopCommandError extends Error {
  constructor(readonly code: IpcError['code']) {
    super(code)
    this.name = 'DesktopCommandError'
  }
}

export async function unwrap<T>(operation: Promise<{ status: 'ok'; data: T } | { status: 'error'; error: IpcError }>): Promise<T> {
  let result: Awaited<typeof operation>
  try {
    result = await operation
  } catch {
    throw new DesktopCommandError('INTERNAL_ERROR')
  }
  if (result.status === 'error') throw new DesktopCommandError(result.error.code)
  return result.data
}

/** Única frontera de la ventana; los componentes no invocan Tauri directamente. */
export const desktopService = {
  /** Estado de la instalación persistente; el laboratorio responde OPERATION_NOT_AVAILABLE. */
  installationStatus: () => unwrap(commands.installationStatus()),
  listBranches: () => unwrap(commands.labListBranches()),
  listCategories: (offset = 0) => unwrap(commands.labListCategories({ limit: 20, offset })),
  listProducts: (offset = 0) => unwrap(commands.labListProducts({ limit: 20, offset })),
  /** Primera escritura auditada; solo disponible en el laboratorio sintético. */
  createProduct: (request: CreateProductRequest_Deserialize) => unwrap(commands.labCreateProduct(request)),

  /**
   * Autenticación sobre la instalación persistente.
   *
   * El handle de sesión no viaja al renderer: el proceso Rust lo custodia y los
   * comandos posteriores lo usan sin que la UI pueda leerlo ni almacenarlo.
   */
  authLogin: (email: string, password: string) => unwrap(commands.authLogin({ email, password })),
  authMe: () => unwrap(commands.authMe()),
  authLogout: () => unwrap(commands.authLogout()),
  authLogoutAll: () => unwrap(commands.authLogoutAll()),
  authSelectBranch: (branchId: string) => unwrap(commands.authSelectBranch({ branchId })),
  authListBranches: () => unwrap(commands.authListBranches()),
}
