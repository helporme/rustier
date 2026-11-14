import { fileURLToPath } from 'node:url'
import { defineConfig } from 'tsdown'
import rustier from 'rustier/tsdown'

export default defineConfig({
  entry: { index: 'src/index.ts' },
  dts: true,
  deps: {
    neverBundle: ['rustier', 'rolldown', 'vite'],
  },
  plugins: [rustier({
    platform: { type: 'node' },
    procMacros: true,
    crates: {
      rustier_workspace: { path: fileURLToPath(new URL('../../crates/workspace', import.meta.url)) },
      rustier_compiler: { path: fileURLToPath(new URL('../../crates/compiler', import.meta.url)) },
    },
  })],
})
