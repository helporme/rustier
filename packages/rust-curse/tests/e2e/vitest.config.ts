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

const OUT_DIR = resolvePath('.rust-curse')
const RUST_INPUT_RE = /\.(rs|toml|lock)$/
const isRustInput = (file: string) =>
  RUST_INPUT_RE.test(file) && !file.startsWith(OUT_DIR)

type Profile = 'wasm' | 'node'

const profiles: Profile[] = ['node', 'wasm']
const outputs: Partial<Record<Profile, BuildOutput>> = {}
const dirtyProfiles = new Set<Profile>(profiles)

const rebuildProfiles = (
  profileNames: Profile[],
  warn: (diag: string) => void,
) => {
  const output = curse.build(profileNames)

  for (const profile of profileNames) {
    outputs[profile] = output
    dirtyProfiles.delete(profile)
  }

  for (const diag of output.diags) {
    warn(diag)
  }
}

// use this convoulted plugin setup since vitest sends file change updates only to the root plugin
// and not to the per project plugins
const rustCurseWatcher = (): Plugin => ({
  name: 'rust-curse-watcher',

  configureServer(server) {
    server.watcher.on('change', (file) => {
      if (!isRustInput(file)) return

      rebuildProfiles(profiles, (diag) => server.config.logger.warn(diag))
    })
  },
})

const rustCurse = (profile: Profile): Plugin => {
  return {
    name: 'rust-curse-' + profile,

    buildStart() {
      rebuildProfiles([profile], (diag) => this.warn(diag))
    },

    resolveId(id) {
      if (Object.values(VIRTUAL_MODULES).includes(id))
        return RESOLVED_MOD_PREFIX + id
    },

    watchChange(id) {
      if (isRustInput(id)) {
        dirtyProfiles.add(profile)
      }
    },

    load(id) {
      // Resolve warnigs module
      if (id.substring(1) === VIRTUAL_MODULES.warns) {
        const diags = (outputs[profile]?.diags ?? []).map((diag) => ({
          code: '',
          message: diag,
          rendered: diag,
        }))
        return `export const rustCurseDiags = ${JSON.stringify(diags)};`
      }

      // Resolve .rs files
      if (!id.endsWith('.rs')) return null

      if (!outputs[profile] || dirtyProfiles.has(profile)) {
        rebuildProfiles([profile], (diag) => this.warn(diag))
      }

      return curse.resolve(profile, id)
    },
  }
}

export default defineConfig({
  plugins: [rustCurseWatcher()],
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
