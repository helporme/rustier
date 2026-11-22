import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'tsdown'
import rustier from 'rustier-bootstrap/tsdown'

export default defineConfig({
  entry: { index: 'src/rs.ts', tsdown: 'src/index.ts' },
  dts: true,
  deps: {
    neverBundle: ['rustier', 'rolldown'],
  },
  plugins: [{
    name: 'raw-js',
    load(id) {
      if (!id.endsWith('.js?raw')) return
      const path = id.slice(0, -'?raw'.length)
      this.addWatchFile(path)
      return { code: readFileSync(path, 'utf8'), moduleType: 'text' }
    },
  }, rustier({
    targets: [
      'aarch64-apple-darwin',
      'x86_64-apple-darwin',
      'x86_64-unknown-linux-gnu',
      'aarch64-unknown-linux-gnu',
      'x86_64-unknown-linux-musl',
      'aarch64-unknown-linux-musl',
      'x86_64-pc-windows-msvc',
      'aarch64-pc-windows-msvc',
    ],
    distribution: 'npm',
    concurrency: 1,
    crates: {
      rustc_version: { version: '0.4' },
      serde_json: { version: '1' },
      rustier_workspace: { path: fileURLToPath(new URL('../../crates/workspace', import.meta.url)) },
      rustier_compiler: { path: fileURLToPath(new URL('../../crates/compiler', import.meta.url)) },
    },
  })],
})
