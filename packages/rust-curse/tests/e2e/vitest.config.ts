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

    load(id) {
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
        },
      },
      {
        plugins: [rustCurse('wasm')],
        test: {
          name: 'wasm',
        },
      },
    ],
  },
})
