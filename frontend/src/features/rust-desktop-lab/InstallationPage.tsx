import { Alert, Box, Container, Stack, Table, TableBody, TableCell, TableRow, Typography } from '@mui/material'
import type { InstallationStatusDto } from '../../shared/rust-local/desktop.generated'

const stateLabels: Record<string, string> = {
  FIRST_INSTALL: 'Primera instalación',
  REOPENED: 'Instalación reabierta',
}

/**
 * Modo producto: informa únicamente sobre la instalación persistente.
 * No muestra datos de negocio porque Auth, RBAC y esquema siguen pendientes de aprobación.
 */
export function InstallationPage({ status }: { status: InstallationStatusDto }) {
  return <Container component="main" maxWidth="md" sx={{ py: 4 }}>
    <Stack spacing={3}>
      <Typography variant="h4" component="h1">HiQNet POS — Instalación Local</Typography>
      <Alert severity="warning">
        Previsualización técnica. Las operaciones comerciales requieren la aprobación de las
        decisiones de identidad, catálogo, caja, ventas e inventario.
      </Alert>
      <Alert severity="info" role="status">
        Identidad no habilitada: provisioning, login y operaciones administrativas siguen bloqueados.
        El estado del esquema no acredita un administrador ni una sesión válida; tampoco permite
        distinguir todavía una sesión inexistente, expirada o revocada.
      </Alert>
      <Box component="section" aria-labelledby="installation-heading">
        <Typography id="installation-heading" variant="h6" component="h2">Estado del esquema</Typography>
        <Table aria-label="Estado de la instalación">
          <TableBody>
            <TableRow><TableCell>Apertura</TableCell><TableCell>{stateLabels[status.state] ?? status.state}</TableCell></TableRow>
            <TableRow><TableCell>Versión de esquema</TableCell><TableCell>{status.schemaVersion}</TableCell></TableRow>
            <TableRow><TableCell>Migraciones aplicadas</TableCell><TableCell>{status.appliedMigrations}</TableCell></TableRow>
          </TableBody>
        </Table>
      </Box>
      <Typography color="text.secondary">
        Los datos permanecen en el directorio indicado al iniciar y se conservan entre reinicios.
      </Typography>
    </Stack>
  </Container>
}
