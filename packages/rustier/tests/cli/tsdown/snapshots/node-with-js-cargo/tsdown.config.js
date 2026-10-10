import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: ['src/index.*'],
  format: ['esm'],
  outDir: 'lib',
  sourcemap: true,
  plugins: [{ name: 'existing-plugin' }],
})
