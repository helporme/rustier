import { defineConfig, Plugin } from 'vitest/config'
import { buildNapi, BuildOutput, RustCurse } from 'rust-curse/unstable'
import { resolve as resolvePath } from 'node:path'
import { WasmDeployTarget } from 'rust-curse-native'

// temporary setup
const curse = new RustCurse(
  {
    projectRoot: resolvePath('.'),
    outDir: '.rust-curse',
    rust: { modulesRootDir: 'src' },
    js: {
      filter: {
        include: ['src/**/*.ts', 'src/**/*.js'],
      },
      typescript: true,
    },
    profiles: [
      {
        name: 'wasm',
        rust: { target: { type: 'wasm', deployTo: WasmDeployTarget.NodeJs } },
      },
      {
        name: 'node',
        rust: { target: { type: 'node' } },
      },
    ],
  },
  buildNapi,
)

const VIRTUAL_MODULES = {
  warns: `$build/warns`,
}
const RESOLVED_MOD_PREFIX = '\0'

const rustCurse = (target: 'wasm' | 'node'): Plugin => {
  let output: BuildOutput | undefined

  return {
    name: 'rust-curse-' + target,

    async buildStart() {
      // fix it one day
      // output = await curse.build([target])
      output = await curse.build(['wasm', 'node'])

      for (const warn of output.warns) {
        this.warn({
          code: warn.code,
          message: warn.rendered ?? warn.message,
        })
      }
    },

    resolveId(id) {
      if (Object.values(VIRTUAL_MODULES).includes(id))
        return RESOLVED_MOD_PREFIX + id
    },

    async handleHotUpdate({ file, server, modules }) {
      if (!/\.(rs|toml|lock)$/.test(file)) return
      await curse?.build(['wasm', 'node'])
      server.moduleGraph.invalidateAll()
      return []
    },

    load(id) {
      // Resolve warnigs module
      if (id.substring(1) === VIRTUAL_MODULES.warns) {
        const warns = (output?.warns ?? []).map((w) => ({
          code: w.code,
          message: w.message,
          sourceCodePath: w.sourceCodePath,
          label: w.label,
          help: w.help,
          rendered: w.rendered,
        }))
        return `export const rustCurseWarns = ${JSON.stringify(warns)};`
      }

      // Resolve .rs files
      if (!id.endsWith('.rs') || !output) return null
      return output.resolve(target, id)
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
