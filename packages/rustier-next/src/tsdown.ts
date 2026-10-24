import type { Plugin } from 'rolldown'
import { Server } from 'server-next'

export type Platform = {
  type: 'wasm'
  loading: 'web' | 'node-js'
}

export type CrateDependency =
  | string
  | {
      version?: string
      path?: string
      git?: string
      branch?: string
      tag?: string
      rev?: string
      package?: string
      features?: string[]
      'default-features'?: boolean
    }

export interface RustierOptions {
  release?: boolean
  crates?: Record<string, CrateDependency>
  platform: Platform
}

const runtimeId = '\0rustier-next:runtime'
const valuesId = '\0rustier-next:values'

export default function rustier(options: RustierOptions): Plugin {
  if (
    options.platform?.type !== 'wasm' ||
    !['web', 'node-js'].includes(options.platform.loading)
  ) {
    throw new Error('Rustier requires a wasm platform with web or node-js loading')
  }

  const server = new Server({
    rootDir: process.cwd(),
    release: options.release ?? false,
    crates: options.crates ?? {},
  })
  let wasm: string

  return {
    name: 'rustier-next',
    buildStart() {
      wasm = this.emitFile({
        type: 'asset',
        fileName: 'rustier.wasm',
        source: new Uint8Array(),
      })
    },
    resolveId(id) {
      if (id === 'rustier-next:runtime') return runtimeId
      if (id === 'rustier-next:values') return valuesId
    },
    load(id) {
      if (id === valuesId) return server.jsRuntime()
      if (id !== runtimeId) return

      const loading =
        options.platform.loading === 'node-js'
          ? `import { readFileSync } from 'node:fs';
const instance = new WebAssembly.Instance(
  new WebAssembly.Module(readFileSync(new URL(import.meta.ROLLUP_FILE_URL_${wasm}, import.meta.url))),
  imports
);`
          : `const { instance } = await WebAssembly.instantiate(
  await (await fetch(new URL(import.meta.ROLLUP_FILE_URL_${wasm}, import.meta.url))).arrayBuffer(),
  imports
);`

      return `
import * as values from 'rustier-next:values';
const imports = { './rustier-wasm-rt.mjs': values };
${loading}
values.__rustier_js_init_memory(instance.exports.memory);
export function invoke(name, arguments_) {
  const handles = arguments_.map(value => values.__rustier_js_insert(value));
  try {
    const result = instance.exports[name](0, ...handles);
    return values.__rustier_js_take(result);
  } finally {
    for (const handle of handles) values.__rustier_js_drop_ref(handle);
  }
}
`
    },
    transform(code, id) {
      if (id.startsWith('\0') || id.includes('/node_modules/')) return
      if (/\.d\.[cm]?ts(?:[?#]|$)/.test(id)) return
      if (!/\.[cm]?[jt]sx?(?:[?#]|$)/.test(id)) return
      const transformed = server.transform(id, code)
      if (transformed != null) return { code: transformed, map: null }
    },
    generateBundle(_options, bundle) {
      const modules = [...this.getModuleIds()]
      const fileName = this.getFileName(wasm)
      if (!modules.includes(runtimeId)) {
        delete bundle[fileName]
        return
      }
      server.retainModules(modules)
      const asset = bundle[fileName]
      if (asset.type !== 'asset') throw new Error('Rustier Wasm asset is missing')
      asset.source = server.build()
    },
  }
}
