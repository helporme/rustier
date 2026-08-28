import { defineConfig, type Plugin } from 'vitest/config'
import {
  Server,
  type ProfileBuildOutput,
  WasmTarget,
} from 'rust-curse/unstable'
import { readFileSync } from 'node:fs'
import { resolve as resolvePath } from 'node:path'

type TestTarget = 'wasm' | 'node'
const targets: TestTarget[] = ['node', 'wasm']
const server = new Server({
  manifestPath: resolvePath('Cargo.toml'),
  outDir: resolvePath('.rust-curse'),
  included: ['*.rs'],
  excluded: ['.git', '.hg', '.svn', 'node_modules', 'target'],
  includeDepCrates: [],
  excludeDepCrates: [],
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
server.installDeps()

const VIRTUAL_MODULES = {
  diags: `$build/diags`,
}
const RESOLVED_MOD_PREFIX = '\0'

const OUT_DIR = resolvePath('.rust-curse')
const RUST_INPUT_RE = /\.(rs|toml|lock)$/
const isRustInput = (file: string) =>
  RUST_INPUT_RE.test(file) && !file.startsWith(OUT_DIR)

const updateServer = (
  filePath: string,
  event: 'create' | 'update' | 'delete',
) => {
  if (!isRustInput(filePath)) return false

  server.update([
    {
      filePath,
      ...(event === 'delete'
        ? {}
        : { contents: readFileSync(filePath, 'utf8') }),
    },
  ])
  for (const target of targets) dirtyTargets.add(target)
  return true
}

type CachedOutput = Partial<Pick<ProfileBuildOutput, 'artifact'>> & {
  diags: string[]
}
const outputs: Partial<Record<TestTarget, CachedOutput>> = {}
const dirtyTargets = new Set<TestTarget>(targets)

const reportException = (error: unknown, warn: (diag: string) => void) => {
  const diag = error instanceof Error ? error.message : String(error)
  warn(diag)
  return diag
}

const rebuild = (names: TestTarget[], warn: (diag: string) => void) => {
  try {
    const output = server.build(names)

    for (const name of names) {
      const profileOutput = output.perProfile[name]
      if (profileOutput) {
        outputs[name] = profileOutput
        for (const diag of profileOutput.diags) warn(diag)
      }
      dirtyTargets.delete(name)
    }
  } catch (error) {
    const diag = reportException(error, warn)

    for (const name of names) {
      outputs[name] = {
        artifact: outputs[name]?.artifact,
        diags: [...(outputs[name]?.diags ?? []), diag],
      }
      dirtyTargets.delete(name)
    }
  }
}

// Vitest sends file changes to the root plugin rather than project plugins.
const rustCurseWatcher = (): Plugin => ({
  name: 'rust-curse-watcher',

  configureServer(server) {
    server.watcher.on('change', (file) => {
      if (updateServer(file, 'update'))
        rebuild(targets, (diag) => server.config.logger.warn(diag))
    })
    server.watcher.on('add', (file) => {
      if (updateServer(file, 'create'))
        rebuild(targets, (diag) => server.config.logger.warn(diag))
    })
    server.watcher.on('unlink', (file) => {
      if (updateServer(file, 'delete'))
        rebuild(targets, (diag) => server.config.logger.warn(diag))
    })
  },
})

const rustCurse = (target: TestTarget): Plugin => {
  return {
    name: 'rust-curse-' + target,

    buildStart() {
      rebuild([target], (diag) => this.warn(diag))
    },

    resolveId(id) {
      if (Object.values(VIRTUAL_MODULES).includes(id))
        return RESOLVED_MOD_PREFIX + id
    },

    watchChange(id, change) {
      updateServer(id, change.event)
    },

    load(id) {
      // Resolve diagnostics module
      if (id.substring(1) === VIRTUAL_MODULES.diags) {
        const diags = outputs[target]?.diags ?? []
        return `export const rustCurseDiags = ${JSON.stringify(diags)};`
      }

      // Resolve .rs files
      if (!id.endsWith('.rs')) return null

      if (!outputs[target] || dirtyTargets.has(target)) {
        rebuild([target], (diag) => this.warn(diag))
      }

      if (!outputs[target]?.artifact) {
        const diags = outputs[target]?.diags ?? []
        const diag =
          diags[diags.length - 1] ??
          `profile \`${target}\` has no build artifacts`
        return `throw new Error(${JSON.stringify(diag)});`
      }

      try {
        return server.load(target, id)
      } catch (error) {
        const diag = reportException(error, (diag) => this.warn(diag))
        outputs[target] = {
          artifact: outputs[target]?.artifact,
          diags: [...(outputs[target]?.diags ?? []), diag],
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
          include: ['src/**/test.ts', 'src/**/node.ts'],
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
          include: ['src/**/test.ts', 'src/**/wasm.ts'],
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
