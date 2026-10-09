import { defineConfig, type Plugin } from 'vitest/config'
import rustier, { type RustierTarget } from 'rustier/rolldown'


let activeProfile: 'wasm' | 'node' = 'wasm';
let plugin: any = rustier({
  profiles: {
    wasm: {
      target: { type: 'wasm', loading: 'node-module' },
    },
    node: {
      target: "node"
    }
  },
  activeProfile: () => activeProfile
});

const rustierVitest = (target: 'wasm' | 'node'): Plugin => {
  return {
    ...plugin,
    load(id) {
      activeProfile = target
      return plugin.load?.call(this, id)
    }
  }
}

const diagnosticsPlugin: Plugin = {
  name: "rustier-test-diagnostics",
  resolveId(source) {
    if (source === "$build/diags") {
      return "\0" + source
    }
    return null;
  },
  load(id) {
    if (id === "\0$build/diags") {
      // FIXME: ugly as hell
      return `export const rustierDiags = ${JSON.stringify(plugin.api.server.check())};`
    }
    return null;
  }
}

export default defineConfig({
  plugins: [],
  test: {
    projects: [
      {
        plugins: [rustierVitest('node'), diagnosticsPlugin],
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
        plugins: [rustierVitest('wasm'), diagnosticsPlugin],
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
