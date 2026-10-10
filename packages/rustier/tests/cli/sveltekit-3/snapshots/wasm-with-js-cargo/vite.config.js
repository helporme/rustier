import adapter from '@sveltejs/adapter-auto'
import { sveltekit } from '@sveltejs/kit/vite'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [
    sveltekit({
      adapter: adapter(),
      compilerOptions: {
        experimental: {
          async: true,
        },
      },
    }),
    { name: 'existing-plugin' },
  ],
  server: {
    port: 4173,
  },
})
