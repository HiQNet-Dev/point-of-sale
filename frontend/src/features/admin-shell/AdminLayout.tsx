import AppBar from '@mui/material/AppBar'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Divider from '@mui/material/Divider'
import Drawer from '@mui/material/Drawer'
import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemIcon from '@mui/material/ListItemIcon'
import ListItemText from '@mui/material/ListItemText'
import Toolbar from '@mui/material/Toolbar'
import Typography from '@mui/material/Typography'
import { useState } from 'react'
import { Link, NavLink, Outlet, useLocation } from 'react-router-dom'
import { useAuthSession } from '../auth/model/useAuthSession'

const NAV_ITEMS = [
  { label: 'Artículos', icon: 'inventory', to: '/admin/inventory/items' },
  { label: 'Existencias por sucursal', icon: 'storefront', to: '/admin/inventory/stock' },
  { label: 'Recetas', icon: 'kitchen', to: '/admin/inventory/recipes' },
  { label: 'Efectos de modificadores', icon: 'settings-applications', to: '/admin/inventory/modifier-effects' },
  { label: 'Configuración', icon: 'settings', to: '/admin/inventory/settings' },
]

const DRAWER_WIDTH = 240

function DrawerHeading() {
  return (
    <>
      <Toolbar sx={{ px: 2 }}>
        <Typography component="p" variant="h6">
          HiQNet
        </Typography>
      </Toolbar>
      <Divider />
    </>
  )
}

function DrawerNavigation({ onNavigate }: { onNavigate?: () => void }) {
  const location = useLocation()
  return (
    <Box component="nav" aria-label="Secciones de administración" sx={{ p: 1 }}>
      <List disablePadding>
        {NAV_ITEMS.map((item) => (
          <ListItemButton
            key={item.to}
            component={NavLink}
            to={item.to}
            selected={location.pathname.startsWith(item.to)}
            onClick={onNavigate}
            sx={{ borderRadius: 1, p: 0.75 }}
          >
            <ListItemIcon
              sx={{ minWidth: { xs: 32, md: 36 }, color: 'action.active', display: 'flex', alignItems: 'center' }}
              aria-hidden="true"
            >
              <Box sx={{ fontSize: 18 }}>
                {item.icon === 'inventory' && '📦'}
                {item.icon === 'storefront' && '🏪'}
                {item.icon === 'kitchen' && '👨‍🍳'}
                {item.icon === 'settings-applications' && '⚙️'}
                {item.icon === 'settings' && '⚙️'}
              </Box>
            </ListItemIcon>
            <ListItemText primary={item.label} />
          </ListItemButton>
        ))}
      </List>
    </Box>
  )
}

export function AdminLayout() {
  const { me, logout } = useAuthSession()
  const [mobileOpen, setMobileOpen] = useState(false)

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      <Drawer
        variant="temporary"
        open={mobileOpen}
        onClose={() => setMobileOpen(false)}
        sx={{
          display: { xs: 'block', md: 'none' },
          '& .MuiDrawer-paper': { width: DRAWER_WIDTH, boxSizing: 'border-box' },
        }}
      >
        <DrawerHeading />
        <DrawerNavigation onNavigate={() => setMobileOpen(false)} />
      </Drawer>

      <Drawer
        variant="permanent"
        sx={{
          display: { xs: 'none', md: 'block' },
          width: DRAWER_WIDTH,
          flexShrink: 0,
          '& .MuiDrawer-paper': { width: DRAWER_WIDTH, boxSizing: 'border-box' },
        }}
      >
        <DrawerHeading />
        <DrawerNavigation />
      </Drawer>

      <Box sx={{ flexGrow: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <AppBar position="static">
          <Toolbar sx={{ gap: 1 }}>
            <Button
              color="inherit"
              onClick={() => setMobileOpen(true)}
              aria-label="Abrir navegación"
              sx={{ display: { xs: 'inline-flex', md: 'none' }, minWidth: 0 }}
            >
              Menú
            </Button>
            <Typography variant="subtitle1" component="h1" sx={{ flexGrow: 1, fontWeight: 600 }}>
              Administración
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ display: { xs: 'none', sm: 'block' } }}>
              {me?.email}
            </Typography>
            <Button component={Link} to="/pos" size="small">
              Punto de venta
            </Button>
            <Button size="small" onClick={() => void logout()}>
              Salir
            </Button>
          </Toolbar>
        </AppBar>
        <Box component="main" sx={{ flexGrow: 1, p: { xs: 2, md: 3 }, minWidth: 0 }}>
          <Outlet />
        </Box>
      </Box>
    </Box>
  )
}
