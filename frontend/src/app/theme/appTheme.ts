import { createTheme } from '@mui/material/styles'
import type { ThemeOptions } from '@mui/material/styles'

/**
 * Tokens del sistema de diseño de HiQNet POS.
 *
 * Se centralizan aquí los colores, la tipografía, el espaciado, los radios y
 * los estados interactivos para que toda la interfaz comparta una identidad
 * visual coherente y previsible. Los componentes deben consumir estos tokens a
 * través del tema (`theme.palette`, `theme.spacing`, `sx`) en lugar de valores
 * sueltos, y nunca fijar colores o tamaños arbitrarios por pantalla.
 *
 * Identidad: azul institucional como color de acción principal, superficie
 * clara con contraste suficiente para uso prolongado en punto de venta y una
 * escala tipográfica compacta pensada para densidad de datos.
 */
export const designTokens = {
  /** Azul institucional HiQNet: color de acción y foco. */
  brand: {
    main: '#1a4f8b',
    light: '#4a7ab5',
    dark: '#0d3663',
  },
  /** Superficies y textos neutros de alto contraste. */
  neutral: {
    background: '#eef1f5',
    surface: '#ffffff',
    border: 'rgba(27, 42, 61, 0.14)',
    textPrimary: '#182230',
    textSecondary: '#55606e',
  },
  /** Semánticos de estado alineados con los chips de la aplicación. */
  status: {
    success: '#1b7f4b',
    warning: '#b25a06',
    error: '#b42318',
  },
  /** Radios y elevación comunes. */
  shape: {
    radius: 8,
    radiusLarge: 12,
  },
} as const

const options: ThemeOptions = {
  palette: {
    mode: 'light',
    primary: {
      main: designTokens.brand.main,
      light: designTokens.brand.light,
      dark: designTokens.brand.dark,
      contrastText: '#ffffff',
    },
    secondary: { main: '#0f766e' },
    success: { main: designTokens.status.success },
    warning: { main: designTokens.status.warning },
    error: { main: designTokens.status.error },
    info: { main: designTokens.brand.main },
    background: {
      default: designTokens.neutral.background,
      paper: designTokens.neutral.surface,
    },
    text: {
      primary: designTokens.neutral.textPrimary,
      secondary: designTokens.neutral.textSecondary,
    },
    divider: designTokens.neutral.border,
  },
  shape: { borderRadius: designTokens.shape.radius },
  spacing: 8,
  typography: {
    fontFamily: [
      '"Segoe UI"',
      'system-ui',
      '-apple-system',
      'Roboto',
      '"Helvetica Neue"',
      'Arial',
      'sans-serif',
    ].join(','),
    h1: { fontSize: '1.75rem', fontWeight: 700, lineHeight: 1.2 },
    h2: { fontSize: '1.375rem', fontWeight: 700, lineHeight: 1.25 },
    h3: { fontSize: '1.125rem', fontWeight: 700, lineHeight: 1.3 },
    h4: { fontSize: '1.5rem', fontWeight: 700, lineHeight: 1.25 },
    h5: { fontSize: '1.25rem', fontWeight: 700, lineHeight: 1.3 },
    h6: { fontSize: '1rem', fontWeight: 700, lineHeight: 1.35 },
    subtitle1: { fontWeight: 600 },
    subtitle2: { fontWeight: 600 },
    button: { fontWeight: 600, textTransform: 'none', letterSpacing: 0 },
    overline: { fontWeight: 700, letterSpacing: '0.08em', lineHeight: 1.6 },
    caption: { lineHeight: 1.4 },
  },
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        // Foco visible y consistente para navegación por teclado.
        ':focus-visible': {
          outline: `2px solid ${designTokens.brand.main}`,
          outlineOffset: 2,
        },
        body: { backgroundColor: designTokens.neutral.background },
      },
    },
    MuiButton: {
      defaultProps: { disableElevation: true },
      styleOverrides: {
        root: { borderRadius: designTokens.shape.radius, paddingInline: 16 },
        // Alturas mínimas para uso táctil en tablet y móvil sin perder densidad.
        sizeSmall: { minHeight: 36 },
        sizeMedium: { minHeight: 40 },
        sizeLarge: { minHeight: 48 },
        contained: { '&:hover': { boxShadow: 'none' } },
      },
    },
    MuiPaper: {
      styleOverrides: {
        root: { backgroundImage: 'none' },
      },
    },
    MuiAppBar: {
      defaultProps: { elevation: 0, color: 'default' },
      styleOverrides: {
        root: {
          backgroundColor: designTokens.neutral.surface,
          borderBottom: `1px solid ${designTokens.neutral.border}`,
        },
      },
    },
    MuiToolbar: {
      styleOverrides: {
        root: { minHeight: 56 },
      },
    },
    MuiDialog: {
      styleOverrides: {
        paper: { borderRadius: designTokens.shape.radiusLarge },
      },
    },
    MuiDialogTitle: {
      styleOverrides: {
        root: { fontSize: '1.125rem', fontWeight: 700 },
      },
    },
    MuiDialogActions: {
      styleOverrides: {
        root: { padding: '12px 24px' },
      },
    },
    MuiOutlinedInput: {
      styleOverrides: {
        root: { borderRadius: designTokens.shape.radius },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: { borderRadius: designTokens.shape.radius, fontWeight: 600 },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: { borderRadius: designTokens.shape.radius },
      },
    },
    MuiAccordion: {
      defaultProps: { disableGutters: true, elevation: 0 },
      styleOverrides: {
        root: {
          border: `1px solid ${designTokens.neutral.border}`,
          borderRadius: designTokens.shape.radius,
          '&:before': { display: 'none' },
        },
      },
    },
    MuiAccordionSummary: {
      styleOverrides: {
        root: { minHeight: 44 },
        content: { marginBlock: 8, fontWeight: 600 },
      },
    },
  },
}

/** Tema único de la aplicación; consumido por `AppProviders`. */
export const appTheme = createTheme(options)
