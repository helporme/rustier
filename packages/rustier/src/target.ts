import { type ServerPlatform, ServerWasmLoading } from 'rustier-native'

export type RustierTarget =
  | 'node'
  | 'wasm'
  | { type: 'node' }
  | {
      type: 'wasm'
      /** Defaults to "fetch", including when target is the shorthand "wasm". */
      loading?: 'fetch' | 'node-module' | 'module'
    }

export function toServerPlatform(target: RustierTarget): ServerPlatform {
  const resolved = typeof target === 'string' ? { type: target } : target
  switch (resolved?.type) {
    case 'node':
      return { type: 'Node' }
    case 'wasm': {
      const loading =
        'loading' in resolved ? (resolved.loading ?? 'fetch') : 'fetch'
      switch (loading) {
        case 'fetch':
          return { type: 'Wasm', target: ServerWasmLoading.Fetch }
        case 'node-module':
          return { type: 'Wasm', target: ServerWasmLoading.ModuleNode }
        case 'module':
          return { type: 'Wasm', target: ServerWasmLoading.Module }
        default:
          throw new Error(
            'rustier: wasm loading must be "fetch", "node-module", or "module"',
          )
      }
    }
    default:
      throw new Error(
        'rustier: target must be "node", "wasm", or an object with type "node" or "wasm"',
      )
  }
}
