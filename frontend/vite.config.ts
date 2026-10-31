import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // Im Dev-Modus gehen API-Aufrufe an das lokal laufende ECS-Backend.
    proxy: { '/api': 'http://localhost:3000' },
  },
})
