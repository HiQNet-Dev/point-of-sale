import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import Chip from '@mui/material/Chip'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import {
  BASE_UNIT_LABELS,
  ITEM_STATUS_LABELS,
  listInventoryItems,
} from '../api/inventoryItemsApi'
import type { InventoryBaseUnit, InventoryItemStatus } from '../api/inventoryItemsApi'
import { inventoryItemsKeys } from '../model/inventoryItemsKeys'

/** Minimal inventory item shape needed to render a picker selection. */
export interface SelectedInventoryItem {
  id: string
  code: string
  name: string
  baseUnit: InventoryBaseUnit
  fractionalUnitAllowed: boolean
  status: InventoryItemStatus
}

export interface InventoryItemPickerProps {
  organizationId: string
  value: SelectedInventoryItem | null
  onChange: (item: SelectedInventoryItem | null) => void
  /** Items already used by sibling rows (kept selectable only on their own row). */
  disabledItemIds?: string[]
  label?: string
  disabled?: boolean
  error?: boolean
  helperText?: string
  autoFocus?: boolean
}

const OPTION_PAGE_SIZE = 50

function itemLabel(item: SelectedInventoryItem): string {
  return `${item.name} · ${item.code}`
}

/**
 * Server-searched selector over the organization's InventoryItem catalog.
 * New selections are limited to ACTIVE items; an already-selected item that is
 * now INACTIVE is still injected and rendered so historical references survive.
 */
export function InventoryItemPicker({
  organizationId,
  value,
  onChange,
  disabledItemIds,
  label = 'Artículo',
  disabled = false,
  error = false,
  helperText,
  autoFocus = false,
}: InventoryItemPickerProps) {
  const [input, setInput] = useState('')

  const query = useQuery({
    queryKey: inventoryItemsKeys.list(organizationId, {
      page: 0,
      size: OPTION_PAGE_SIZE,
      q: input,
      status: 'ACTIVE',
    }),
    queryFn: () =>
      listInventoryItems(organizationId, {
        page: 0,
        size: OPTION_PAGE_SIZE,
        q: input,
        status: 'ACTIVE',
      }),
    enabled: Boolean(organizationId) && !disabled,
  })

  const options = useMemo(() => {
    const loaded: SelectedInventoryItem[] = (query.data?.items ?? []).map((item) => ({
      id: item.id,
      code: item.code,
      name: item.name,
      baseUnit: item.baseUnit,
      fractionalUnitAllowed: item.fractionalUnitAllowed,
      status: item.status,
    }))
    if (value && !loaded.some((option) => option.id === value.id)) {
      return [value, ...loaded]
    }
    return loaded
  }, [query.data, value])

  return (
    <Autocomplete
      options={options}
      value={value}
      inputValue={input}
      onInputChange={(_event, next) => setInput(next)}
      onChange={(_event, next) => onChange(next)}
      getOptionLabel={itemLabel}
      isOptionEqualToValue={(option, candidate) => option.id === candidate.id}
      getOptionDisabled={(option) =>
        Boolean(disabledItemIds?.includes(option.id)) && option.id !== value?.id
      }
      disabled={disabled}
      loading={query.isPending}
      loadingText="Cargando artículos…"
      noOptionsText="Sin artículos activos"
      autoHighlight
      openOnFocus
      size="small"
      renderOption={(props, option) => {
        const { key, ...rest } = props as { key: string }
        return (
          <Box component="li" key={key} {...rest} sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
            <Typography variant="body2" sx={{ flexGrow: 1 }}>
              {option.name} · {option.code}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {BASE_UNIT_LABELS[option.baseUnit]}
            </Typography>
            <Chip
              size="small"
              label={ITEM_STATUS_LABELS[option.status]}
              color={option.status === 'ACTIVE' ? 'success' : 'default'}
            />
          </Box>
        )
      }}
      renderInput={(params) => (
        <TextField
          {...params}
          label={label}
          error={error}
          helperText={helperText}
          autoFocus={autoFocus}
        />
      )}
    />
  )
}
