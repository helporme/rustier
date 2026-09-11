import { defineConfig, type Plugin } from 'vitest/config'
import rustCurse, { type RustCurseTarget } from 'rust-curse/rolldown'


let activeProfile: 'wasm' | 'node' = 'wasm';
let plugin: any = rustCurse({
  profiles: {
    wasm: {
      target: 'wasm-node',
    },
    node: {
      target: "node"
    }
  },
  activeProfile: () => activeProfile
});

const rustCurseVitest = (target: 'wasm' | 'node'): Plugin => {
  return {
    ...plugin,
    load(id) {
      activeProfile = target
      return plugin.load?.call(this, id)
    }
  }
}

const diagnosticsPlugin: Plugin = {
  name: "rust-curse-test-diagnostics",
  resolveId(source) {
    if (source === "$build/diags") {
      return "\0" + source
    }
    return null;
  },
  load(id) {
    if (id === "\0$build/diags") {
      // FIXME: ugly as hell
      return `export const rustCurseDiags = ${JSON.stringify(plugin.api.server.check())};`
    }
    return null;
  }
}

export default defineConfig({
  plugins: [],
  test: {
    projects: [
      {
        plugins: [rustCurseVitest('node'), diagnosticsPlugin],
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
        plugins: [rustCurseVitest('wasm'), diagnosticsPlugin],
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
