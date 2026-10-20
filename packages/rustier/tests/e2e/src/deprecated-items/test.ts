import { expect, it } from 'vitest'
import { ServerDiagnosticSeverity } from 'rustier-native'
import { rustierDiags } from '$build/diags'
import * as bindings from './test.rs'

it('keeps current and non-deprecated doc-hidden APIs usable', () => {
  const example = bindings.Example.create(7)

  expect(bindings.current()).toBe(42)
  expect(example.current()).toBe(7)
  expect(example.hidden()).toBe(8)
  expect(bindings.Example.of({ value: 9 }).current()).toBe(9)
})

it('omits deprecated functions and methods from runtime bindings', () => {
  expect(bindings).not.toHaveProperty('old_function')
  expect(bindings.Example).not.toHaveProperty('new')
  expect(bindings.Example).not.toHaveProperty('old_static')
  expect(bindings.Example.create(7)).not.toHaveProperty('old')
})

it('does not use a deprecated new method as the JS constructor', () => {
  expect(() => new (bindings.Example as any)()).toThrow(TypeError)
})

it('warns when skipping deprecated free, inherent, and trait methods', () => {
  const diagnostics = rustierDiags.filter(
    (diag) =>
      diag.code === 'rustier::skipped_fn' &&
      diag.message.includes('e2e::deprecated_items::'),
  )

  const names = [
    'old_function',
    'Example::new',
    'Example::old',
    'Example::old_static',
    'Legacy::old_trait',
  ]

  expect(diagnostics).toHaveLength(names.length)
  for (const name of names) {
    const matches = diagnostics.filter((diag) =>
      diag.message.endsWith(
        `"e2e::deprecated_items::${name}" cannot be exported to JS`,
      ),
    )

    expect(matches).toHaveLength(1)
    const diag = matches[0]!
    expect(diag.severity).toBe(ServerDiagnosticSeverity.Warning)
    expect(diag.loc?.line).toBeGreaterThan(0)
    expect(diag.rendered).toContain('rustier::deprecated_item')
    expect(diag.rendered).toContain(
      'deprecated functions and methods are not exported to JS',
    )
  }
})
