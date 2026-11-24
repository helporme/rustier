import { resolve } from 'node:path'

export interface WorkspaceOptions {
  root: string
  crates: Record<string, ({ path: string } | { version: string }) & { features?: string[] }>
}

export function resolveCrates(root: string, crates: WorkspaceOptions['crates'] = {}): WorkspaceOptions['crates'] {
  return Object.fromEntries(Object.entries(crates).map(([name, crate]) => [
    name, 'path' in crate ? { ...crate, path: resolve(root, crate.path) } : crate,
  ]))
}
