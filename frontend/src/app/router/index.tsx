import { Navigate, createBrowserRouter } from 'react-router-dom'
import { HomePage } from '../pages/HomePage'

/**
 * Route table. Modules under src/modules register their own routes later;
 * this foundation only mounts the product landing view.
 */
export const router = createBrowserRouter([
  {
    path: '/',
    element: <HomePage />,
  },
  {
    path: '*',
    element: <Navigate to="/" replace />,
  },
])