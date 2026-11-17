import { fileURLToPath } from 'node:url'
import { defineConfig } from 'tsdown'
import rustier from 'rustier/tsdown'

export default defineConfig({
  entry: { index: 'src/index.ts' },
  dts: true,
  deps: {
    neverBundle: ['rustier', 'rolldown'],
  },
  plugins: [rustier({
    platform: { type: 'node' },
    procMacros: true,
    release: true,
    crates: {
      serde_json: { version: '1' },
      rustier_workspace: { path: fileURLToPath(new URL('../../crates/workspace', import.meta.url)) },
      rustier_compiler: { path: fileURLToPath(new URL('../../crates/compiler', import.meta.url)) },
    },
  })],
})
