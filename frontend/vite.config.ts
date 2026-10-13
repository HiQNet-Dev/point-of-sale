import react from '@vitejs/plugin-react'
import { configDefaults, defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // Listen on all interfaces so the dev server is reachable from the host
    // when it runs inside the Docker Compose frontend container.
    host: true,
    port: 5173,
    watch: {
      // Bind-mount file events do not always propagate on Windows + Docker
      // Desktop. Polling is opt-in (VITE_USE_POLLING=true) to avoid an
      // aggressive default on Linux/macOS.
      usePolling: process.env.VITE_USE_POLLING === 'true',
    },
  },
  test: {
    // Tests default to Node; browser-bound auth tests opt into jsdom locally.
    environment: 'node',
    exclude: [...configDefaults.exclude, 'e2e/**'],
  },
})
