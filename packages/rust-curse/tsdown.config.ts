import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: ['src/index.ts', 'src/vite/index.ts'],
  exports: {
    packageJson: false,
  },
  dts: true,
  publint: true,
  deps: {
    neverBundle: [/\.node$/, 'rust-curse-native', '@napi-rs/cli'],
  },
})
