import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Entrada separada: las pantallas Java no reciben el transporte sintético.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist-rust-lab',
    rollupOptions: { input: 'rust-lab.html' },
  },
})
