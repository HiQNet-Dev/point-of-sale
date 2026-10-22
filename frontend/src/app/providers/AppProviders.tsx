import CssBaseline from '@mui/material/CssBaseline'
import { ThemeProvider } from '@mui/material/styles'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import type { ReactNode } from 'react'
import { AuthSessionProvider } from '../../features/auth/model/AuthSessionProvider'
import { ApiRequestError } from '../../shared/api/client'
import { appTheme as theme } from '../theme/appTheme'

/**
 * Application-wide providers: MUI theme and TanStack Query.
 * No global state library yet; query cache + local state are sufficient.
 */
export function AppProviders({ children }: { children: ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            retry: (failureCount, error) => {
              if (error instanceof ApiRequestError && error.status < 500) return false
              return failureCount < 1
            },
          },
        },
      }),
  )

  return (
    <QueryClientProvider client={queryClient}>
      <AuthSessionProvider>
        <ThemeProvider theme={theme}>
          <CssBaseline />
          {children}
        </ThemeProvider>
      </AuthSessionProvider>
    </QueryClientProvider>
  )
}