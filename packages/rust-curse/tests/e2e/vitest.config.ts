import { defineConfig, Plugin } from 'vitest/config'
import {
  Driver,
  type ProfileBuildOutput,
  WasmTarget,
} from 'rust-curse/unstable'
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
  diags: `$build/diags`,
}
const RESOLVED_MOD_PREFIX = '\0'

const OUT_DIR = resolvePath('.rust-curse')
const RUST_INPUT_RE = /\.(rs|toml|lock)$/
const isRustInput = (file: string) =>
  RUST_INPUT_RE.test(file) && !file.startsWith(OUT_DIR)

type Profile = 'wasm' | 'node'

const profiles: Profile[] = ['node', 'wasm']
type CachedProfileOutput = Partial<Pick<ProfileBuildOutput, 'artifact'>> & {
  diags: string[]
}

const outputs: Partial<Record<Profile, CachedProfileOutput>> = {}
let sharedDiags: string[] = []
const dirtyProfiles = new Set<Profile>(profiles)

const reportException = (error: unknown, warn: (diag: string) => void) => {
  const diag = error instanceof Error ? error.message : String(error)
  warn(diag)
  return diag
}

const rebuildProfiles = (
  profileNames: Profile[],
  warn: (diag: string) => void,
) => {
  try {
    const output = curse.build(profileNames)
    sharedDiags = output.diags

    for (const diag of output.diags) {
      warn(diag)
    }

    for (const profile of profileNames) {
      const profileOutput = output.perProfile[profile]
      if (profileOutput) {
        outputs[profile] = profileOutput
        for (const diag of profileOutput.diags) {
          warn(diag)
        }
      }
      dirtyProfiles.delete(profile)
    }
  } catch (error) {
    const diag = reportException(error, warn)

    for (const profile of profileNames) {
      outputs[profile] = {
        artifact: outputs[profile]?.artifact,
        diags: [...(outputs[profile]?.diags ?? []), diag],
      }
      dirtyProfiles.delete(profile)
    }
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
      // Resolve diagnostics module
      if (id.substring(1) === VIRTUAL_MODULES.diags) {
        const diags = [...sharedDiags, ...(outputs[profile]?.diags ?? [])]
        return `export const rustCurseDiags = ${JSON.stringify(diags)};`
      }

      // Resolve .rs files
      if (!id.endsWith('.rs')) return null

      if (!outputs[profile] || dirtyProfiles.has(profile)) {
        rebuildProfiles([profile], (diag) => this.warn(diag))
      }

      if (!outputs[profile]?.artifact) {
        const diags = [...sharedDiags, ...(outputs[profile]?.diags ?? [])]
        const diag =
          diags[diags.length - 1] ??
          `profile \`${profile}\` has no build artifacts`
        return `throw new Error(${JSON.stringify(diag)});`
      }

      try {
        return curse.resolve(profile, id)
      } catch (error) {
        const diag = reportException(error, (diag) => this.warn(diag))
        outputs[profile] = {
          artifact: outputs[profile]?.artifact,
          diags: [...(outputs[profile]?.diags ?? []), diag],
        }
        return `throw new Error(${JSON.stringify(diag)});`
      }
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
          include: ['src/**/*.test.ts'],
          exclude: ['src/**/*.wasm.test.ts'],
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
          include: ['src/**/*.test.ts'],
          exclude: ['src/**/*.node.test.ts'],
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
