import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [{ name: 'existing-plugin' }],
  server: {
    port: 4173,
  },
  build: {
    outDir: 'build',
  },
})
