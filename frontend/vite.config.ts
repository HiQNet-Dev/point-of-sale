import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  test: {
    // Pure unit tests only at this stage (no DOM components yet).
    environment: 'node',
  },
})