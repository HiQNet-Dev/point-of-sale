import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { Alert, Box, Button, Container, MenuItem, Stack, TextField, Typography } from '@mui/material'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'
import type { BootstrapStatusDto } from '../../shared/rust-local/desktop.generated'

const platformNotes: Record<string, { title: string; detail: string; action: string }> = {
  AWAITING_AUTHORIZATION: {
    title: 'Falta la autorización de instalación',
    detail:
      'Ejecuta el emisor nativo sobre el directorio de esta instalación y vuelve a comprobar. El token no se escribe en la interfaz.',
    action: 'pos-local --provision-token-issue <directorio-de-instalación>',
  },
  AUTHORIZATION_EXPIRED: {
    title: 'La autorización caducó',
    detail: 'Emite una autorización nueva; la anterior ya no puede consumirse.',
    action: 'pos-local --provision-token-issue <directorio-de-instalación>',
  },
  AUTHORIZATION_REVOKED: {
    title: 'La autorización fue revocada',
    detail: 'Emite una autorización nueva para continuar con la instalación.',
    action: 'pos-local --provision-token-issue <directorio-de-instalación>',
  },
  AUTHORIZATION_CONSUMED: {
    title: 'La autorización ya se usó',
    detail:
      'Otra instalación consumió este derecho. Si la instalación quedó incompleta, emite una autorización nueva.',
    action: 'pos-local --provision-token-issue <directorio-de-instalación>',
  },
  INVALID_AUTHORIZATION: {
    title: 'La autorización no es válida',
    detail: 'El archivo no coincide con un derecho vivo de esta instalación.',
    action: 'pos-local --provision-token-issue <directorio-de-instalación>',
  },
  INSECURE_PERMISSIONS: {
    title: 'Permisos de archivo inseguros',
    detail:
      'El directorio o el archivo de autorización conceden acceso a otros usuarios. Corrige los permisos antes de continuar.',
    action: 'chmod 700 <directorio-de-instalación> && chmod 600 <directorio>/provisioning-authorization.txt',
  },
  UNSUPPORTED_PLATFORM: {
    title: 'Instalación guiada no disponible en Windows todavía',
    detail:
      'La custodia de archivos de Windows requiere el componente nativo de DACL y owner, que aún no está aprobado. La instalación permanece cerrada por diseño.',
    action: '',
  },
}

function messageFor(error: unknown): string {
  if (error instanceof DesktopCommandError) {
    switch (error.code) {
      case 'UNAUTHORIZED':
        return 'La autorización no está vigente.'
      case 'FORBIDDEN':
        return 'Los permisos del archivo de autorización son inseguros.'
      case 'VALIDATION_ERROR':
        return 'Revisa los datos: la contraseña debe tener al menos 12 caracteres.'
      case 'CONFLICT':
        return 'La instalación ya está configurada.'
      case 'OPERATION_NOT_AVAILABLE':
        return 'La instalación guiada no está disponible en esta plataforma.'
      default:
        return 'No se pudo completar la configuración.'
    }
  }
  return 'No se pudo completar la configuración.'
}

/**
 * Asistente de primera instalación.
 *
 * Solo los estados recuperables muestran formulario. Un fallo nunca convierte la
 * instalación en «nueva»: el estado lo decide Rust y se vuelve a consultar.
 */
export function SetupWizard({ status, onCompleted }: { status: BootstrapStatusDto; onCompleted: () => void }) {
  const [businessName, setBusinessName] = useState('')
  const [administratorName, setAdministratorName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmation, setConfirmation] = useState('')
  const [currency, setCurrency] = useState('MXN')
  const [localError, setLocalError] = useState<string | null>(null)

  const install = useMutation({
    mutationFn: () =>
      desktopService.bootstrapInstall({
        businessName,
        administratorName,
        email,
        password,
        currency,
      }),
    onSuccess: () => {
      setPassword('')
      setConfirmation('')
      onCompleted()
    },
    onError: () => {
      setPassword('')
      setConfirmation('')
    },
  })

  if (status.state !== 'READY') {
    const note = platformNotes[status.state] ?? {
      title: 'Estado de instalación no reconocido',
      detail: 'Vuelve a abrir la aplicación para consultar el estado real.',
      action: '',
    }
    return (
      <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
        <Stack spacing={3}>
          <Typography variant="h4" component="h1">Configuración inicial</Typography>
          <Alert severity={status.state === 'INSECURE_PERMISSIONS' ? 'error' : 'info'}>
            <Typography component="p" sx={{ fontWeight: 600 }}>{note.title}</Typography>
            <Typography component="p">{note.detail}</Typography>
            {note.action ? (
              <Box component="pre" sx={{ mt: 1, mb: 0, whiteSpace: 'pre-wrap', fontSize: 13 }}>
                {note.action}
              </Box>
            ) : null}
          </Alert>
          <Button variant="outlined" onClick={onCompleted}>Volver a comprobar</Button>
        </Stack>
      </Container>
    )
  }

  const expires = status.expiresAt
    ? new Date(status.expiresAt * 1000).toLocaleTimeString('es-MX')
    : null

  return (
    <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
      <Stack spacing={3}>
        <Typography variant="h4" component="h1">Primera instalación</Typography>
        <Typography color="text.secondary">
          Configura el negocio y su administrador. La autorización se valida en Rust y nunca se
          escribe en la interfaz.
          {expires ? ` Ventana vigente hasta las ${expires}.` : ''}
        </Typography>
        <Box
          component="form"
          aria-label="Primera instalación"
          onSubmit={(event) => {
            event.preventDefault()
            if (install.isPending) return
            if (password !== confirmation) {
              setLocalError('Las contraseñas no coinciden.')
              return
            }
            if (password.length < 12) {
              setLocalError('La contraseña debe tener al menos 12 caracteres.')
              return
            }
            setLocalError(null)
            install.mutate()
          }}
        >
          <Stack spacing={2}>
            <TextField label="Nombre del negocio" value={businessName} onChange={(event) => setBusinessName(event.target.value)} required fullWidth />
            <TextField label="Administrador" value={administratorName} onChange={(event) => setAdministratorName(event.target.value)} required fullWidth />
            <TextField label="Correo del administrador" type="email" value={email} onChange={(event) => setEmail(event.target.value)} required fullWidth />
            <TextField label="Contraseña inicial" type="password" autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} required fullWidth helperText="Mínimo 12 caracteres" />
            <TextField label="Repite la contraseña" type="password" autoComplete="new-password" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} required fullWidth />
            <TextField select label="Moneda" value={currency} onChange={(event) => setCurrency(event.target.value)} fullWidth>
              <MenuItem value="MXN">MXN — Peso mexicano</MenuItem>
              <MenuItem value="USD">USD — Dólar</MenuItem>
              <MenuItem value="EUR">EUR — Euro</MenuItem>
            </TextField>
            <Button type="submit" variant="contained" disabled={install.isPending}>
              {install.isPending ? 'Configurando…' : 'Crear instalación'}
            </Button>
          </Stack>
        </Box>
        {localError ? <Alert severity="error" role="alert">{localError}</Alert> : null}
        {install.isError ? <Alert severity="error" role="alert">{messageFor(install.error)}</Alert> : null}
      </Stack>
    </Container>
  )
}
