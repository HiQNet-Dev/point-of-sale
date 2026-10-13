import { Navigate, createBrowserRouter } from 'react-router-dom'
import { AdminLayout } from '../../features/admin-shell/AdminLayout'
import { LoginRoute } from '../../features/auth/components/LoginRoute'
import { ProtectedRoute } from '../../features/auth/components/ProtectedRoute'
import { LoginPage } from '../../features/auth/pages/LoginPage'
import { SessionHomePage } from '../../features/auth/pages/SessionHomePage'
import { InventoryItemsPage } from '../../features/inventory-items/pages/InventoryItemsPage'
import { BranchStockDetailPage } from '../../features/inventory-stock/pages/BranchStockDetailPage'
import { BranchStockPage } from '../../features/inventory-stock/pages/BranchStockPage'
import { PosSessionPage } from '../../features/pos-session/pages/PosSessionPage'

/**
 * Public login plus protected session and POS routes. Business modules add
 * their own protected routes as they are implemented.
 */
export const router = createBrowserRouter([
  {
    path: '/login',
    element: (
      <LoginRoute>
        <LoginPage />
      </LoginRoute>
    ),
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <SessionHomePage />
      </ProtectedRoute>
    ),
  },
  {
    path: '/pos',
    element: (
      <ProtectedRoute>
        <PosSessionPage />
      </ProtectedRoute>
    ),
  },
  {
    path: '/admin',
    element: (
      <ProtectedRoute>
        <AdminLayout />
      </ProtectedRoute>
    ),
    children: [
      { index: true, element: <Navigate to="/admin/inventory/items" replace /> },
      { path: 'inventory/items', element: <InventoryItemsPage /> },
      { path: 'inventory/stock', element: <BranchStockPage /> },
      { path: 'inventory/stock/:itemId', element: <BranchStockDetailPage /> },
    ],
  },
  {
    path: '*',
    element: <Navigate to="/" replace />,
  },
])
