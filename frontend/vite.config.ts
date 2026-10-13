import react from '@vitejs/plugin-react'
import { configDefaults, defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Read VITE_* variables from the repository root `.env` — the same file
  // Compose and the seed/reset scripts use — so the documented host flow
  // (`cp .env.example .env` then `pnpm dev`) needs no extra frontend/.env.
  // Existing process.env VITE_* values (Compose, CI) still take precedence.
  envDir: '..',
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
