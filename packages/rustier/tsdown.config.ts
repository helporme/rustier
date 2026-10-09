import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    unstable: 'src/unstable/index.ts',
    rolldown: 'src/rolldown.ts',
    tsdown: 'src/tsdown.ts',
    vite: 'src/vite.ts',
  },
  exports: {
    packageJson: false,
  },
  dts: true,
  publint: true,
  deps: {
    neverBundle: [/\.node$/, 'rustier-native', '@napi-rs/cli'],
  },
})
