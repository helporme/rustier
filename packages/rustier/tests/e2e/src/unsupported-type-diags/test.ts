import { expect, it } from 'vitest'
import { ServerDiagnosticSeverity } from 'rustier-native'
import { rustierDiags } from '$build/diags'
import * as bindings from './test.rs'

const diagnostics = rustierDiags.filter((diag) =>
  diag.message.includes('e2e::unsupported_type_diags::'),
)

it('keeps supported exports alongside unsupported types', () => {
  expect(bindings.supported()).toBe(42)
  expect(bindings).not.toHaveProperty('Text')
  expect(bindings).not.toHaveProperty('length')
  expect(bindings).not.toHaveProperty('Token')
  expect(bindings).not.toHaveProperty('make')
})

it('diagnoses both an unsized declaration and a function borrowing it', () => {
  for (const [code, name] of [
    ['rustier::skipped_struct', 'Text'],
    ['rustier::skipped_fn', 'length'],
  ]) {
    const matches = diagnostics.filter(
      (diag) =>
        diag.code === code &&
        diag.message.includes(`e2e::unsupported_type_diags::${name}`),
    )

    expect(matches).toHaveLength(1)
    const diag = matches[0]!
    expect(diag.severity).toBe(ServerDiagnosticSeverity.Warning)
    expect(diag.loc?.line).toBeGreaterThan(0)
    expect(diag.rendered).toContain('rustier::unsupported_unsized_type')
    expect(diag.rendered).toContain('Text')
  }
})

it('diagnoses the unsupported declaration path behind a public re-export', () => {
  const matches = diagnostics.filter(
    (diag) =>
      diag.code === 'rustier::skipped_fn' &&
      diag.message.includes('e2e::unsupported_type_diags::make'),
  )

  expect(matches).toHaveLength(1)
  const diag = matches[0]!
  expect(diag.severity).toBe(ServerDiagnosticSeverity.Warning)
  expect(diag.loc?.line).toBeGreaterThan(0)
  expect(diag.rendered).toContain('rustier::inaccessible_type')
  expect(diag.rendered).toContain('Token')
  expect(diag.rendered).toContain('re-export paths are not supported yet')
})
