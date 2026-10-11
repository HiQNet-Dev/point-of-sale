import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Container from '@mui/material/Container'
import Divider from '@mui/material/Divider'
import Paper from '@mui/material/Paper'
import Stack from '@mui/material/Stack'
import Typography from '@mui/material/Typography'
import { Link } from 'react-router-dom'
import { useAuthSession } from '../model/useAuthSession'

export function SessionHomePage() {
  const { me, logout } = useAuthSession()
  if (!me) return null

  return (
    <Box component="main" sx={{ minHeight: '100dvh', py: { xs: 4, sm: 8 }, px: 2 }}>
      <Container maxWidth="sm" disableGutters>
        <Paper variant="outlined" sx={{ p: { xs: 3, sm: 4 } }}>
          <Stack spacing={3}>
            <Box>
              <Typography component="p" variant="overline" color="primary.main" sx={{ fontWeight: 700 }}>
                HiQNet POS
              </Typography>
              <Typography component="h1" variant="h4" sx={{ mt: 0.5 }}>
                Session active
              </Typography>
            </Box>

            <Box>
              <Typography variant="body2" color="text.secondary">Signed in as</Typography>
              <Typography variant="body1" sx={{ fontWeight: 600 }}>{me.email}</Typography>
            </Box>

            <Button component={Link} to="/pos" variant="contained">
              Abrir punto de venta
            </Button>

            <Divider />

            <Box component="dl" sx={{ display: 'grid', gridTemplateColumns: 'minmax(110px, 1fr) 2fr', gap: 1, m: 0 }}>
              <Typography component="dt" variant="body2" color="text.secondary">Currency</Typography>
              <Typography component="dd" variant="body2" sx={{ m: 0 }}>{me.organization.currency}</Typography>
              <Typography component="dt" variant="body2" color="text.secondary">Locale</Typography>
              <Typography component="dd" variant="body2" sx={{ m: 0 }}>{me.organization.locale}</Typography>
              <Typography component="dt" variant="body2" color="text.secondary">Timezone</Typography>
              <Typography component="dd" variant="body2" sx={{ m: 0, overflowWrap: 'anywhere' }}>
                {me.organization.timezone}
              </Typography>
            </Box>

            <Button variant="outlined" onClick={() => void logout()}>
              Sign out
            </Button>
          </Stack>
        </Paper>
      </Container>
    </Box>
  )
}
