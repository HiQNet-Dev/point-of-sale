import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, Container, MenuItem, Stack, Tab, Table, TableBody, TableCell, TableRow, Tabs, TextField, Typography } from '@mui/material'
import { DesktopCommandError, desktopService } from '../../shared/rust-local/desktop-client'
import type { BranchDto, InstallationStatusDto, SessionIdentityDto } from '../../shared/rust-local/desktop.generated'
import { LoginPage } from './LoginPage'
import { SetupWizard } from './SetupWizard'
import { CatalogWorkspace } from './CatalogWorkspace'
import { BusinessSettingsPage } from './BusinessSettingsPage'
import { BranchesPage } from './BranchesPage'
import { TeamPage } from './TeamPage'

const stateLabels: Record<string, string> = {
  FIRST_INSTALL: 'Primera instalación',
  REOPENED: 'Instalación reabierta',
}

function codeOf(error: unknown): string | null {
  return error instanceof DesktopCommandError ? error.code : null
}

/**
 * Modo producto: exige sesión para mostrar cualquier dato de la instalación.
 *
 * Una sesión vencida o revocada vuelve a la pantalla de acceso; el proceso Rust
 * ya descartó el handle nativo en ese caso.
 */
export function SessionGate({ status }: { status: InstallationStatusDto }) {
  const queryClient = useQueryClient()
  const bootstrap = useQuery({
    queryKey: ['rust-desktop', 'bootstrap'],
    queryFn: desktopService.bootstrapStatus,
    retry: false,
  })
  const provisioned = bootstrap.data?.state === 'ALREADY_PROVISIONED'
  const me = useQuery({
    queryKey: ['rust-desktop', 'auth'],
    queryFn: desktopService.authMe,
    retry: false,
    enabled: provisioned,
  })

  if (bootstrap.isPending) {
    return <Container component="main" sx={{ py: 4 }}><Typography role="status">Comprobando la instalación local…</Typography></Container>
  }
  if (bootstrap.isError) {
    return <Container component="main" sx={{ py: 4 }}>
      <Alert severity="error" role="alert">No se pudo consultar la instalación: {bootstrap.error.message}</Alert>
    </Container>
  }
  if (!provisioned) {
    return <SetupWizard
      status={bootstrap.data}
      onCompleted={() => void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'bootstrap'] })}
    />
  }
  if (me.isPending) {
    return <Container component="main" sx={{ py: 4 }}><Typography role="status">Comprobando la sesión local…</Typography></Container>
  }
  if (me.isError) {
    if (codeOf(me.error) === 'UNAUTHORIZED') return <LoginPage />
    return <Container component="main" sx={{ py: 4 }}>
      <Alert severity="error" role="alert">No se pudo comprobar la sesión: {me.error.message}</Alert>
    </Container>
  }
  return <AuthenticatedHome identity={me.data} status={status} />
}

/**
 * Sesión autenticada: identidad, permisos efectivos, sucursal y cierre.
 *
 * Los permisos mostrados provienen de SQLite en cada consulta; la UI no decide
 * autorización.
 */
function AuthenticatedHome({ identity, status }: { identity: SessionIdentityDto; status: InstallationStatusDto }) {
  const queryClient = useQueryClient()
  const [section, setSection] = useState<'operation' | 'business' | 'branches' | 'team'>('operation')
  const branches = useQuery({
    queryKey: ['rust-desktop', 'branches'],
    queryFn: desktopService.authListBranches,
    retry: false,
  })
  const selectBranch = useMutation({
    mutationFn: (branchId: string) => desktopService.authSelectBranch(branchId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'auth'] })
      void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'branches'] })
    },
  })
  const logout = useMutation({
    mutationFn: () => desktopService.authLogout(),
    // Al cerrar sesión se descarta toda caché administrativa: los datos de una
    // sesión anterior no deben reaparecer ante otra cuenta.
    onSuccess: () => queryClient.removeQueries({ queryKey: ['rust-desktop'] }),
  })
  const logoutAll = useMutation({
    mutationFn: () => desktopService.authLogoutAll(),
    onSuccess: () => queryClient.removeQueries({ queryKey: ['rust-desktop'] }),
  })

  const branchesUnavailable = branches.isError && codeOf(branches.error) === 'FORBIDDEN'

  return (
    <Container component="main" maxWidth="md" sx={{ py: 4 }}>
      <Stack spacing={3}>
        <Stack direction="row" spacing={2} sx={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <Box>
            <Typography variant="h4" component="h1">HiQNet POS — Local</Typography>
            <Typography color="text.secondary">{identity.displayName} · {identity.email}</Typography>
          </Box>
          <Stack direction="row" spacing={1}>
            <Button variant="outlined" onClick={() => logout.mutate()} disabled={logout.isPending}>
              Cerrar sesión
            </Button>
            <Button color="warning" onClick={() => logoutAll.mutate()} disabled={logoutAll.isPending}>
              Cerrar todas
            </Button>
          </Stack>
        </Stack>

        {logout.isError || logoutAll.isError ? (
          <Alert severity="error" role="alert">No se pudo cerrar la sesión.</Alert>
        ) : null}

        <Tabs
          value={section}
          onChange={(_, value: 'operation' | 'business' | 'branches' | 'team') => setSection(value)}
          aria-label="Secciones de la aplicación"
          variant="scrollable"
          allowScrollButtonsMobile
        >
          <Tab value="operation" label="Operación" id="section-operation" aria-controls="panel-operation" />
          <Tab value="business" label="Negocio" id="section-business" aria-controls="panel-business" />
          <Tab value="branches" label="Sucursales" id="section-branches" aria-controls="panel-branches" />
          <Tab value="team" label="Equipo" id="section-team" aria-controls="panel-team" />
        </Tabs>

        {section === 'business' ? (
          <Box role="tabpanel" id="panel-business" aria-labelledby="section-business">
            <BusinessSettingsPage permissions={identity.permissions} />
          </Box>
        ) : null}

        {section === 'branches' ? (
          <Box role="tabpanel" id="panel-branches" aria-labelledby="section-branches">
            <BranchesPage permissions={identity.permissions} />
          </Box>
        ) : null}

        {section === 'team' ? (
          <Box role="tabpanel" id="panel-team" aria-labelledby="section-team">
            <TeamPage permissions={identity.permissions} currentUserId={identity.userId} />
          </Box>
        ) : null}

        {section === 'operation' ? (
          <Box role="tabpanel" id="panel-operation" aria-labelledby="section-operation">
            <Stack spacing={3}>
              <Box component="section" aria-labelledby="session-heading">
                <Typography id="session-heading" variant="h6" component="h2">Sesión</Typography>
                <Table aria-label="Identidad autenticada">
                  <TableBody>
                    <TableRow><TableCell>Organización</TableCell><TableCell>{identity.organizationId}</TableCell></TableRow>
                    <TableRow><TableCell>Permisos efectivos</TableCell><TableCell>{identity.permissions.length}</TableCell></TableRow>
                    <TableRow><TableCell>Sucursal activa</TableCell><TableCell>{identity.branchId ?? 'Sin seleccionar'}</TableCell></TableRow>
                    <TableRow><TableCell>Esquema local</TableCell><TableCell>{stateLabels[status.state] ?? status.state} · v{status.schemaVersion} ({status.appliedMigrations} migraciones)</TableCell></TableRow>
                  </TableBody>
                </Table>
              </Box>

              <Box component="section" aria-labelledby="branch-heading">
                <Typography id="branch-heading" variant="h6" component="h2">Sucursal operativa</Typography>
                {branchesUnavailable ? (
                  <Alert severity="info">La cuenta no tiene permiso para enumerar sucursales.</Alert>
                ) : branches.isError ? (
                  <Alert severity="error" role="alert">No se pudieron leer las sucursales.</Alert>
                ) : branches.isPending ? (
                  <Typography role="status">Cargando sucursales…</Typography>
                ) : branches.data.length === 0 ? (
                  <Alert severity="info">La organización todavía no tiene sucursales activas.</Alert>
                ) : (
                  <TextField
                    select
                    label="Sucursal"
                    value={identity.branchId ?? ''}
                    onChange={(event) => selectBranch.mutate(event.target.value)}
                    disabled={selectBranch.isPending}
                    fullWidth
                  >
                    <MenuItem value="" disabled>Selecciona una sucursal</MenuItem>
                    {branches.data.map((branch: BranchDto) => (
                      <MenuItem key={branch.id} value={branch.id} disabled={branch.status !== 'ACTIVE'}>
                        {branch.code} · {branch.name}
                      </MenuItem>
                    ))}
                  </TextField>
                )}
                {selectBranch.isError ? (
                  <Alert severity="error" role="alert" sx={{ mt: 2 }}>
                    {codeOf(selectBranch.error) === 'NOT_FOUND'
                      ? 'La sucursal no pertenece a esta instalación.'
                      : 'No puedes operar en esa sucursal.'}
                  </Alert>
                ) : null}
              </Box>

              <CatalogWorkspace permissions={identity.permissions} />
            </Stack>
          </Box>
        ) : null}
      </Stack>
    </Container>
  )
}
