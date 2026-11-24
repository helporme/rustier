import { posix } from 'node:path'
import { RolldownMagicString } from 'rolldown'
import { parseSync } from 'rolldown/utils'

export function applyEdits(code: string, path: string, edits: [number, number, string][]) {
  if (!edits.length) return
  const result = new RolldownMagicString(code)
  for (const [start, end, text] of edits) {
    if (start === end) result.appendLeft(start, text)
    else result.overwrite(start, end, text)
  }
  return {
    code: result.toString(),
    map: result.generateMap({ source: path, includeContent: true, hires: true }).toString(),
  }
}

export function relativeImport(chunk: string, file: string) {
  const relative = posix.relative(posix.dirname(chunk), file)
  return relative.startsWith('.') ? relative : `./${relative}`
}

export function rewriteImport(code: string, file: string, from: string, to: string) {
  const result = new RolldownMagicString(code)
  for (const statement of parseSync(file, code).program.body) {
    if (statement.type === 'ImportDeclaration' && statement.source.value === from) {
      result.overwrite(statement.source.start, statement.source.end, JSON.stringify(to))
    }
  }
  return { code: result.toString(), map: result.generateMap({ hires: true }).toString() }
}
