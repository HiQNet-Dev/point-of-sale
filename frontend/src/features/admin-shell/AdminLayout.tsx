import AppBar from '@mui/material/AppBar'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Divider from '@mui/material/Divider'
import Drawer from '@mui/material/Drawer'
import List from '@mui/material/List'
import ListItemButton from '@mui/material/ListItemButton'
import ListItemText from '@mui/material/ListItemText'
import Toolbar from '@mui/material/Toolbar'
import Typography from '@mui/material/Typography'
import { Link, NavLink, Outlet, useLocation } from 'react-router-dom'
import { useAuthSession } from '../auth/model/useAuthSession'

const NAV_ITEMS = [
  { label: 'Artículos', to: '/admin/inventory/items' },
  { label: 'Existencias por sucursal', to: '/admin/inventory/stock' },
]

const DRAWER_WIDTH = 240

export function AdminLayout() {
  const { me, logout } = useAuthSession()
  const location = useLocation()

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      <Drawer
        variant="permanent"
        sx={{
          width: DRAWER_WIDTH,
          flexShrink: 0,
          '& .MuiDrawer-paper': { width: DRAWER_WIDTH, boxSizing: 'border-box' },
        }}
      >
        <Toolbar sx={{ px: 2 }}>
          <Typography component="p" variant="h6">
            HiQNet
          </Typography>
        </Toolbar>
        <Divider />
        <Box component="nav" aria-label="Secciones de administración" sx={{ p: 1 }}>
          <List disablePadding>
            {NAV_ITEMS.map((item) => (
              <ListItemButton
                key={item.to}
                component={NavLink}
                to={item.to}
                selected={location.pathname.startsWith(item.to)}
                sx={{ borderRadius: 1 }}
              >
                <ListItemText primary={item.label} />
              </ListItemButton>
            ))}
          </List>
        </Box>
      </Drawer>

      <Box sx={{ flexGrow: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <AppBar position="static" color="default" elevation={0} sx={{ borderBottom: 1, borderColor: 'divider' }}>
          <Toolbar sx={{ gap: 1 }}>
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
