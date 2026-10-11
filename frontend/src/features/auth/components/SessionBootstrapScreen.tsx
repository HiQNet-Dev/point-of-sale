import Box from '@mui/material/Box'
import CircularProgress from '@mui/material/CircularProgress'
import Typography from '@mui/material/Typography'

export function SessionBootstrapScreen() {
  return (
    <Box
      component="main"
      aria-busy="true"
      aria-live="polite"
      sx={{ minHeight: '100dvh', display: 'grid', placeItems: 'center', px: 2 }}
    >
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
        <CircularProgress size={24} aria-label="Loading session" />
        <Typography color="text.secondary">Checking your session…</Typography>
      </Box>
    </Box>
  )
}
