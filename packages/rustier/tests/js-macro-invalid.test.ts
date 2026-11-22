import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterAll, expect, test } from 'vitest'
import { Server } from '../../../crates/server/dist/rustier-server.js'

// Invalid generated modules and invalid dependency sources must not poison the
// shared bridge used by the successful calls.
const rootDir = mkdtempSync(join(tmpdir(), 'rustier-invalid-js-'))
afterAll(() => rmSync(rootDir, { recursive: true, force: true }))

function fixture(body: string, features: string[] = []) {
  const profile = features[0] ?? 'inline'
  const path = fileURLToPath(new URL('./handy-crate', import.meta.url))
  const server = new Server({ rootDir, profiles: {
    [profile]: { backend: { type: 'node' }, procMacros: true, crates: { 'handy-crate': { path, features } } },
  } })
  const source = 'import { rs } from "rustier"; export const run = () => rs`' + body + '`;'
  const result = server.transformModule(profile, '/case.js', source, './bridge.mjs')
  return { server, profile, result }
}

for (const place of ['inline', 'handy-crate']) {
  test(`${place}: invalid JavaScript in js! fails to parse`, () => {
    const { server, profile, result } = place === 'inline'
      ? fixture('js!(const broken = ;)')
      : fixture('handy_crate::invalid_js(__rustier_env)', ['invalid-js'])
    expect(result.diagnostics).toEqual([])
    const source = server.renderJsFfi(profile, './unused.node')
    expect(source).toMatch(/const broken\s*=\s*;/)
    const check = spawnSync(process.execPath, ['--input-type=module', '--check'], { input: source, encoding: 'utf8' })
    expect(check.status).toBe(1)
    expect(check.stderr).toMatch(/SyntaxError: Unexpected token ';'/)
  }, 120_000)
}

for (const place of ['inline', 'handy-crate']) {
  test(`${place}: rejects a nested rs tagged template inside js!`, () => {
    const { server, profile } = place === 'inline'
      ? fixture('js!(return rs\\`1u32\\`)')
      : fixture('handy_crate::nested_rs(__rustier_env)', ['nested-rs'])
    const result = server.build(profile)
    expect(result.binFfiPath).toBeUndefined()
    expect(result.diagnostics.join('\n')).toMatch(/unknown start of token: [`\\]/)
  }, 120_000)
}
