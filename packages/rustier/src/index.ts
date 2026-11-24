export function rs(
  _strings: TemplateStringsArray,
  ..._values: unknown[]
): any {
  throw new Error('rs templates must be compiled by the Rustier plugin')
}
