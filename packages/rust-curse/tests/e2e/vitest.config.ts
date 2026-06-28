import { defineConfig, Plugin } from 'vitest/config'
import { BuildOutput, Driver, WasmTarget } from 'rust-curse/unstable'
import { resolve as resolvePath } from 'node:path'

const curse = new Driver({
  manifestPath: resolvePath('Cargo.toml'),
  outDir: resolvePath('.rust-curse'),
  profiles: {
    wasm: {
      target: { type: 'Wasm', target: WasmTarget.NodeJs },
      release: false,
    },
    node: {
      target: { type: 'Node' },
      release: false,
    },
  },
})
curse.installDeps()

const VIRTUAL_MODULES = {
  warns: `$build/warns`,
}
const RESOLVED_MOD_PREFIX = '\0'

const rustCurse = (profile: 'wasm' | 'node'): Plugin => {
  let output: BuildOutput | undefined

  return {
    name: 'rust-curse-' + profile,

    async buildStart() {
      output = curse.build([profile])

      for (const diag of output.diags) {
        this.warn(diag)
      }
    },

    resolveId(id) {
      if (Object.values(VIRTUAL_MODULES).includes(id))
        return RESOLVED_MOD_PREFIX + id
    },

    async handleHotUpdate({ file, server, modules }) {
      if (!/\.(rs|toml|lock)$/.test(file)) return
      curse?.build(['wasm', 'node'])
      server.moduleGraph.invalidateAll()
      return []
    },

    load(id) {
      // Resolve warnigs module
      if (id.substring(1) === VIRTUAL_MODULES.warns) {
        const diags = (output?.diags ?? []).map((diag) => ({
          code: '',
          message: diag,
          rendered: diag,
        }))
        return `export const rustCurseDiags = ${JSON.stringify(diags)};`
      }

      // Resolve .rs files
      if (!id.endsWith('.rs') || !output) return null
      return curse.resolve(profile, id)
    },
  }
}

export default defineConfig({
  test: {
    projects: [
      {
        plugins: [rustCurse('node')],
        test: {
          name: 'node',
          provide: { target: 'node' },
          typecheck: {
            enabled: true,
            include: ['src/**/*.type-test.ts'],
            tsconfig: './tsconfig.json',
          },
        },
      },
      {
        plugins: [rustCurse('wasm')],
        test: {
          name: 'wasm',
          provide: { target: 'wasm' },
          typecheck: {
            enabled: true,
            include: ['src/**/*.type-test.ts'],
            tsconfig: './tsconfig.json',
          },
        },
      },
    ],
  },
})
