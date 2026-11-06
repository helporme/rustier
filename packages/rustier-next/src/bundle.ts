import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { posix } from 'node:path'
import { RolldownMagicString } from 'rolldown'
import { parseSync } from 'rolldown/utils'
import type { Server, TransformEdit } from 'server-next'

export function applyTransformEdits(code: string, id: string, edits: TransformEdit[]) {
  const result = new RolldownMagicString(code)
  for (const edit of edits) {
    if (edit.start === edit.end) {
      result.appendLeft(edit.start, edit.text)
    } else {
      result.overwrite(edit.start, edit.end, edit.text)
    }
  }
  return {
    code: result.toString(),
    map: result.generateMap({ source: id, includeContent: true, hires: true }).toString(),
  }
}

export function reportDiagnostics(reported: Set<string>, diagnostics: string[], warn: (message: string) => void) {
  for (const diagnostic of diagnostics) {
    if (reported.has(diagnostic)) continue
    reported.add(diagnostic)
    warn(diagnostic)
  }
}

export function buildRuntime(server: Server, profile: string, binaryName: string) {
  const result = server.build(profile)
  if (!result.binFfiPath) throw new Error(result.diagnostics.join('\n') || 'Rustier build failed')
  const binary = readFileSync(result.binFfiPath)
  const runtime = server.renderJsFfi(profile, `./${binaryName}`)
  const hash = createHash('sha256').update(runtime).update(binary).digest('hex').slice(0, 16)
  return { binary, runtime, runtimeFileName: `runtime-${profile}-${hash}.mjs`, diagnostics: result.diagnostics }
}

export function rewriteRuntimeImports(
  code: string,
  fileName: string,
  runtimes: ReadonlyMap<string, string>,
) {
  const result = new RolldownMagicString(code)
  const parsed = parseSync(fileName, code)
  for (const statement of parsed.program.body) {
    if (statement.type === 'ImportDeclaration') {
      const runtimeFileName = runtimes.get(statement.source.value)
      if (!runtimeFileName) continue
      const relative = posix.relative(posix.dirname(fileName), runtimeFileName)
      const path = relative.startsWith('.') ? relative : `./${relative}`
      result.overwrite(statement.source.start, statement.source.end, JSON.stringify(path))
    }
  }
  return { code: result.toString(), map: result.generateMap({ hires: true }).toString() }
}
