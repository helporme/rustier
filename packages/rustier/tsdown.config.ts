import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    cli: 'src/cli/index.ts',
    'cli/bin': 'src/cli/bin.ts',
    unstable: 'src/unstable/index.ts',
    rolldown: 'src/rolldown.ts',
    tsdown: 'src/tsdown.ts',
    vite: 'src/vite.ts',
  },
  exports: {
    packageJson: false,
    exclude: ['cli/bin'],
    bin: 'src/cli/bin.ts',
  },
  dts: true,
  publint: true,
  deps: {
    neverBundle: [/\.node$/, 'rustier-native', '@napi-rs/cli'],
  },
})
