import { commands, type IpcError } from './desktop.generated'

/** Errores públicos del adaptador IPC, sin diagnósticos de almacenamiento. */
export class DesktopCommandError extends Error {
  constructor(readonly code: IpcError['code']) {
    super(code)
    this.name = 'DesktopCommandError'
  }
}

async function unwrap<T>(operation: Promise<{ status: 'ok'; data: T } | { status: 'error'; error: IpcError }>): Promise<T> {
  let result: Awaited<typeof operation>
  try {
    result = await operation
  } catch {
    throw new DesktopCommandError('INTERNAL_ERROR')
  }
  if (result.status === 'error') throw new DesktopCommandError(result.error.code)
  return result.data
}

/** Única frontera de la pantalla; los componentes no invocan Tauri directamente. */
export const desktopCatalogService = {
  listBranches: () => unwrap(commands.labListBranches()),
  listCategories: (offset = 0) => unwrap(commands.labListCategories({ limit: 20, offset })),
  listProducts: (offset = 0) => unwrap(commands.labListProducts({ limit: 20, offset })),
}
