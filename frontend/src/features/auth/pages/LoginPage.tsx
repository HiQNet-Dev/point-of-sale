import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import CircularProgress from '@mui/material/CircularProgress'
import Container from '@mui/material/Container'
import Paper from '@mui/material/Paper'
import Stack from '@mui/material/Stack'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { ApiNetworkError, ApiRequestError } from '../../../shared/api/client'
import { useAuthSession } from '../model/useAuthSession'

function loginErrorMessage(error: unknown): string {
  if (error instanceof ApiRequestError && error.code === 'INVALID_CREDENTIALS') {
    return 'The organization code, email, or password is incorrect.'
  }
  if (error instanceof ApiNetworkError) return 'Unable to connect to the server. Check your connection and try again.'
  if (error instanceof ApiRequestError && error.status >= 500) {
    return 'Sign-in is temporarily unavailable. Try again shortly.'
  }
  return 'Unable to sign in. Check your details and try again.'
}

export function LoginPage() {
  const { login, logoutNotice } = useAuthSession()

  const [organizationCode, setOrganizationCode] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [pending, setPending] = useState(false)
  const [loginError, setLoginError] = useState<unknown>(null)
  const pendingRef = useRef(false)

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (pendingRef.current) return
    pendingRef.current = true
    setPending(true)
    setLoginError(null)
    const request = { organizationCode: organizationCode.trim(), email, password }
    setPassword('')
    try {
      await login(request)
    } catch (error) {
      setLoginError(error)
      pendingRef.current = false
      setPending(false)
    }
  }

  return (
    <Box component="main" sx={{ minHeight: '100dvh', display: 'grid', placeItems: 'center', py: 4, px: 2 }}>
      <Container maxWidth="xs" disableGutters>
        <Paper variant="outlined" sx={{ p: { xs: 3, sm: 4 } }}>
          <Stack spacing={3}>
            <Box>
              <Typography component="p" variant="overline" color="primary.main" sx={{ fontWeight: 700 }}>
                HiQNet POS
              </Typography>
              <Typography component="h1" variant="h4" sx={{ mt: 0.5 }}>
                Sign in
              </Typography>
              <Typography color="text.secondary" sx={{ mt: 1 }}>
                Use your organization account to continue.
              </Typography>
            </Box>

            {logoutNotice && <Alert severity="info">{logoutNotice}</Alert>}
            {loginError !== null && <Alert severity="error" role="alert">{loginErrorMessage(loginError)}</Alert>}

            <Box component="form" onSubmit={submit}>
              <Stack spacing={2.25}>
                <TextField
                  id="organization-code"
                  label="Organization code"
                  value={organizationCode}
                  onChange={(event) => setOrganizationCode(event.target.value)}
                  autoComplete="off"
                  required
                  fullWidth
                  slotProps={{ htmlInput: { maxLength: 64 } }}
                />
                <TextField
                  id="email"
                  label="Email"
                  type="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                  autoComplete="username"
                  required
                  fullWidth
                  slotProps={{ htmlInput: { maxLength: 254 } }}
                />
                <TextField
                  id="password"
                  label="Password"
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  autoComplete="current-password"
                  required
                  fullWidth
                  slotProps={{ htmlInput: { maxLength: 128 } }}
                />
                <Button
                  type="submit"
                  variant="contained"
                  size="large"
                  fullWidth
                  disabled={pending || !organizationCode.trim() || !email || !password}
                  startIcon={pending ? <CircularProgress size={18} color="inherit" /> : undefined}
                >
                  {pending ? 'Signing in…' : 'Sign in'}
                </Button>
              </Stack>
            </Box>
          </Stack>
        </Paper>
      </Container>
    </Box>
  )
}
