import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    unstable: 'src/unstable/index.ts',
    rolldown: 'src/rolldown.ts',
    tsdown: 'src/tsdown.ts',
  },
  exports: {
    packageJson: false,
  },
  dts: true,
  publint: true,
  deps: {
    neverBundle: [/\.node$/, 'rust-curse-native', '@napi-rs/cli'],
  },
})
