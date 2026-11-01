import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Alert, Box, Button, MenuItem, Stack, TextField, Typography } from '@mui/material'
import { desktopService } from '../../shared/rust-local/desktop-client'
import type { OrganizationDto, UpdateOrganizationRequestDto_Deserialize } from '../../shared/rust-local/desktop.generated'
import { describeError } from './errors'

const policyLabels: Record<string, string> = {
  STRICT: 'Estricta — no se permite stock negativo',
  ALLOW_NEGATIVE: 'Permitir stock negativo',
}

const statusLabels: Record<string, string> = {
  ACTIVE: 'Operativo',
  INACTIVE: 'Inactivo',
}

interface FormState {
  name: string
  timezone: string
  locale: string
  negativeStockPolicy: string
}

function toForm(organization: OrganizationDto): FormState {
  return {
    name: organization.name,
    timezone: organization.timezone,
    locale: organization.locale,
    negativeStockPolicy: organization.negativeStockPolicy,
  }
}

/** Solo envía lo que cambió: un campo en su valor actual viaja como ausente. */
function buildRequest(form: FormState, organization: OrganizationDto): UpdateOrganizationRequestDto_Deserialize {
  const base = toForm(organization)
  return {
    expectedRevision: organization.revision,
    name: form.name !== base.name ? form.name : null,
    timezone: form.timezone !== base.timezone ? form.timezone : null,
    locale: form.locale !== base.locale ? form.locale : null,
    negativeStockPolicy:
      form.negativeStockPolicy !== base.negativeStockPolicy ? form.negativeStockPolicy : null,
  }
}

function isDirty(form: FormState, organization: OrganizationDto): boolean {
  const request = buildRequest(form, organization)
  return (
    request.name !== null ||
    request.timezone !== null ||
    request.locale !== null ||
    request.negativeStockPolicy !== null
  )
}

/**
 * Configuración del negocio de la organización autenticada.
 *
 * El formulario parte de los valores leídos de Rust y solo envía lo que cambió,
 * junto con la revisión leída: la autorización, el CAS y la auditoría ocurren en
 * el backend. La organización la deriva Rust de la sesión; la interfaz no la
 * elige ni la envía.
 */
export function BusinessSettingsPage({ permissions }: { permissions: string[] }) {
  const queryClient = useQueryClient()
  const organization = useQuery({
    queryKey: ['rust-desktop', 'organization'],
    queryFn: desktopService.organizationGet,
    retry: false,
  })
  const save = useMutation({
    mutationFn: (request: UpdateOrganizationRequestDto_Deserialize) =>
      desktopService.organizationUpdate(request),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'organization'] })
      // La zona horaria del negocio es la base de la herencia de la sucursal.
      void queryClient.invalidateQueries({ queryKey: ['rust-desktop', 'branches'] })
    },
  })

  if (organization.isPending) {
    return <Typography role="status">Cargando la configuración del negocio…</Typography>
  }
  if (organization.isError) {
    return <Alert severity="error" role="alert">{describeError(organization.error, 'read')}</Alert>
  }

  const data = organization.data
  return (
    <Box component="section" aria-labelledby="business-settings-heading">
      <Stack spacing={3}>
        <Box>
          <Typography id="business-settings-heading" variant="h5" component="h2">
            Configuración del negocio
          </Typography>
          <Typography color="text.secondary">
            {data.name} · {statusLabels[data.status] ?? data.status}
          </Typography>
        </Box>

        {save.isError ? (
          <Alert severity="error" role="alert">{describeError(save.error, 'conflict')}</Alert>
        ) : null}
        {save.isSuccess ? <Alert severity="success">Configuración guardada.</Alert> : null}

        {/* La clave reinicia el borrador cuando el backend devuelve otra revisión. */}
        <OrganizationForm
          key={data.revision}
          organization={data}
          canUpdate={permissions.includes('organization.update')}
          pending={save.isPending}
          onSave={(request) => save.mutate(request)}
        />
      </Stack>
    </Box>
  )
}

interface OrganizationFormProps {
  organization: OrganizationDto
  canUpdate: boolean
  pending: boolean
  onSave: (request: UpdateOrganizationRequestDto_Deserialize) => void
}

function OrganizationForm({ organization, canUpdate, pending, onSave }: OrganizationFormProps) {
  const [form, setForm] = useState<FormState>(() => toForm(organization))
  const dirty = isDirty(form, organization)
  const update = (field: keyof FormState, value: string) => setForm({ ...form, [field]: value })

  return (
    <Stack spacing={3}>
      <Box component="section" aria-labelledby="general-heading">
        <Typography id="general-heading" variant="h6" component="h3" gutterBottom>
          Información general
        </Typography>
        <Stack spacing={2}>
          <TextField
            label="Nombre comercial"
            value={form.name}
            onChange={(event) => update('name', event.target.value)}
            disabled={!canUpdate || pending}
            helperText="Es el nombre que verá el personal en la aplicación."
            fullWidth
          />
          <TextField
            label="Código de negocio"
            value={organization.code}
            disabled
            helperText="El código identifica la instalación y no se puede cambiar."
            fullWidth
          />
          <TextField
            label="Moneda"
            value={organization.currency}
            disabled
            helperText="No editable: cambiarla reinterpretaría importes ya registrados."
            fullWidth
          />
          <TextField
            label="Zona horaria del negocio"
            value={form.timezone}
            onChange={(event) => update('timezone', event.target.value)}
            disabled={!canUpdate || pending}
            helperText="Zona IANA, por ejemplo America/Mexico_City. Las sucursales la heredan si no definen la suya."
            fullWidth
          />
          <TextField
            label="Configuración regional"
            value={form.locale}
            onChange={(event) => update('locale', event.target.value)}
            disabled={!canUpdate || pending}
            helperText="Etiqueta de idioma y región, por ejemplo es-MX."
            fullWidth
          />
          <TextField
            label="Estado"
            value={statusLabels[organization.status] ?? organization.status}
            disabled
            helperText="La baja del negocio todavía no tiene contrato aprobado en la instalación local."
            fullWidth
          />
        </Stack>
      </Box>

      <Box component="section" aria-labelledby="operations-heading">
        <Typography id="operations-heading" variant="h6" component="h3" gutterBottom>
          Operación
        </Typography>
        <TextField
          select
          label="Política de inventario"
          value={form.negativeStockPolicy}
          onChange={(event) => update('negativeStockPolicy', event.target.value)}
          disabled={!canUpdate || pending}
          helperText="Define si una venta puede dejar existencias en negativo."
          fullWidth
        >
          {Object.entries(policyLabels).map(([value, label]) => (
            <MenuItem key={value} value={value}>{label}</MenuItem>
          ))}
        </TextField>
      </Box>

      {!canUpdate ? (
        <Alert severity="info">Tu cuenta puede consultar la configuración, pero no modificarla.</Alert>
      ) : null}

      <Stack direction="row" spacing={2}>
        <Button
          variant="contained"
          onClick={() => onSave(buildRequest(form, organization))}
          disabled={!canUpdate || pending || !dirty}
        >
          {pending ? 'Guardando…' : 'Guardar cambios'}
        </Button>
        <Button
          variant="outlined"
          onClick={() => setForm(toForm(organization))}
          disabled={pending || !dirty}
        >
          Descartar cambios
        </Button>
      </Stack>

      <Typography variant="caption" color="text.secondary">
        Revisión {organization.revision}. Los cambios se registran en la auditoría de la instalación.
      </Typography>
    </Stack>
  )
}
