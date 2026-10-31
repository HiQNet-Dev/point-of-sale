import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, Container, Stack, TextField, Typography } from '@mui/material'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'

/** Mensaje público y genérico: no distingue usuario inexistente de clave errónea. */
function messageFor(error: unknown): string {
  if (error instanceof DesktopCommandError) {
    switch (error.code) {
      case 'UNAUTHORIZED':
        return 'Credenciales no válidas.'
      case 'VALIDATION_ERROR':
        return 'Revisa el correo y la contraseña.'
      case 'CONFLICT':
        return 'Ya hay una sesión activa en esta instalación'
      case 'INTERNAL_ERROR':
        return 'No se pudo completar la autenticación.'
      default:
        return 'No se pudo completar la autenticación.'
    }
  }
  return 'No se pudo completar la autenticación.'
}

/**
 * Pantalla de login de la instalación persistente.
 *
 * La contraseña vive solo en el estado del formulario y se limpia al terminar.
 */
export function LoginPage() {
  const queryClient = useQueryClient()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const login = useMutation({
    mutationFn: () => desktopService.authLogin(email, password),
    onSuccess: () => {
      setPassword('')
      void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'auth'] })
    },
    onError: () => setPassword(''),
  })

  return (
    <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
      <Stack spacing={3}>
        <Typography variant="h4" component="h1">
          HiQNet POS — Acceso local
        </Typography>
        <Typography color="text.secondary">
          Identifícate con el administrador creado durante el provisioning de esta instalación.
        </Typography>
        <Box
          component="form"
          aria-label="Acceso local"
          onSubmit={(event) => {
            event.preventDefault()
            if (!login.isPending) login.mutate()
          }}
        >
          <Stack spacing={2}>
            <TextField
              label="Correo"
              type="email"
              autoComplete="username"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              required
              fullWidth
            />
            <TextField
              label="Contraseña"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
              fullWidth
            />
            <Button type="submit" variant="contained" disabled={login.isPending}>
              {login.isPending ? 'Comprobando…' : 'Entrar'}
            </Button>
          </Stack>
        </Box>
        {login.isError ? <Alert severity="error" role="alert">{messageFor(login.error)}</Alert> : null}
      </Stack>
    </Container>
  )
}
