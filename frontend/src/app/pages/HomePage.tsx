import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import Container from '@mui/material/Container'
import Typography from '@mui/material/Typography'

/**
 * Minimal product landing view. Purpose: verify build, routing, providers
 * and project structure. Not a functional POS screen.
 */
export function HomePage() {
  return (
    <Container maxWidth="md" sx={{ py: 8 }}>
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: 2,
          textAlign: 'center',
        }}
      >
        <Typography variant="h3" component="h1">
          HiQNet POS
        </Typography>
        <Chip label="Foundation — in active development" color="primary" />
        <Typography variant="body1" color="text.secondary">
          Sistema punto de venta modular para pequeños negocios de alimentos y retail ligero.
          Esta vista solo confirma que la base del proyecto compila, enruta y ejecuta sus providers.
        </Typography>
        <Box component="pre" sx={{ typography: 'body2' }}>
          {`backend/  Spring Boot 4 · Java 21 · PostgreSQL · Flyway
frontend/ React · TypeScript · Vite · MUI
docs/     product · architecture · security · development`}
        </Box>
      </Box>
    </Container>
  )
}