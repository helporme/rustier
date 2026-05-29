import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    tsdown: 'src/tsdown/index.ts',
    unstable: 'src/unstable/index.ts',
    utils: 'src/utils.ts',
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
