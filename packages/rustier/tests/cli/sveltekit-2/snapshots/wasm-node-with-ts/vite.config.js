import { sveltekit } from '@sveltejs/kit/vite'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [sveltekit(), { name: 'existing-plugin' }],
  server: {
    port: 4173,
  },
})
