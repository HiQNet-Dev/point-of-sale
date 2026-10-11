import react from '@vitejs/plugin-react'
import { configDefaults, defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  test: {
    // Tests default to Node; browser-bound auth tests opt into jsdom locally.
    environment: 'node',
    exclude: [...configDefaults.exclude, 'e2e/**'],
  },
})