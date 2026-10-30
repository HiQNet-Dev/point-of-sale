import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { CssBaseline, ThemeProvider } from '@mui/material'
import { appTheme } from './app/theme/appTheme'
import { DesktopCatalogPage } from './features/rust-desktop-lab/DesktopCatalogPage'

const queryClient = new QueryClient()
const root = document.getElementById('root')
if (!root) throw new Error('No existe el contenedor de la aplicación')
createRoot(root).render(
  <ThemeProvider theme={appTheme}>
    <CssBaseline />
    <QueryClientProvider client={queryClient}><DesktopCatalogPage /></QueryClientProvider>
  </ThemeProvider>,
)
