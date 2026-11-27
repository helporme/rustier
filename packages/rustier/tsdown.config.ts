import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'tsdown'
import rustier from 'rustier-bootstrap/tsdown'
import { nativePlatforms } from './src/targets.ts'

export default defineConfig(({ env }) => {
  const localRuntime = env?.runtimeSource === 'local'
  return {
    entry: { tsdown: 'src/tsdown/index.ts', vite: 'src/vite/index.ts', index: 'src/index.ts' },
    dts: true,
    deps: {
      neverBundle: ['rustier', 'rolldown', 'vite'],
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
      targets: localRuntime ? ['host'] : Object.keys(nativePlatforms),
      distribution: localRuntime ? 'inline' : 'npm',
      concurrency: 1,
      crates: {
        rustc_version: { version: '0.4' },
        serde_json: { version: '1' },
        rustier_workspace: { path: fileURLToPath(new URL('../../crates/workspace', import.meta.url)) },
        rustier_compiler: {
          path: fileURLToPath(new URL('../../crates/compiler', import.meta.url)),
          features: localRuntime ? ['local-runtime'] : [],
        },
      },
    })],
  }
})
