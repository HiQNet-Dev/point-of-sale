import { useQuery } from '@tanstack/react-query'
import { Alert, Container, Typography } from '@mui/material'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'
import { DesktopCatalogPage } from './DesktopCatalogPage'
import { InstallationPage } from './InstallationPage'

/**
 * Única ventana con dos modos: instalación persistente o laboratorio sintético.
 * El modo lo decide el proceso Rust; la UI solo lo refleja.
 */
export function DesktopShell() {
  const status = useQuery({ queryKey: ['rust-desktop', 'installation'], queryFn: desktopService.installationStatus, retry: false })

  if (status.isPending) {
    return <Container component="main" sx={{ py: 4 }}><Typography role="status">Abriendo la instalación local…</Typography></Container>
  }
  if (status.isError) {
    if (status.error instanceof DesktopCommandError && status.error.code === 'OPERATION_NOT_AVAILABLE') {
      return <DesktopCatalogPage />
    }
    return <Container component="main" sx={{ py: 4 }}>
      <Alert severity="error" role="alert">No se pudo abrir la instalación local: {status.error.message}</Alert>
    </Container>
  }
  return <InstallationPage status={status.data} />
}
