import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: ['src/index.ts'],
  exports: true,
  format: ['es'],
  publint: true,
  deps: {
    neverBundle: [/\.node$/],
  },
  copy: ['../rust-curse-native/*.node'],
})
