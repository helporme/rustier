import { expect, it } from 'vitest'
import { ServerDiagnosticSeverity } from 'rustier-native'
import { rustierDiags } from '$build/diags'
import * as bindings from './test.rs'

const advice = rustierDiags.filter(diag =>
  diag.code === 'rustier::coerce_requires_clone'
  && diag.message.includes('e2e::coerce_clone_advice::'),
)

it('suggests Clone once for each ADT where it enables coerce', () => {
  const names = [
    'NeedsCloneStruct',
    'NeedsCloneTuple',
    'NeedsCloneUnit',
    'NeedsCloneEnum',
    'NeedsCloneOpenEnum',
    'NeedsCloneShadowed',
    'UnsupportedFields',
  ]

  expect(advice.map(diag => diag.message).sort()).toEqual(
    names.map(name => `implement \`Clone\` for "e2e::coerce_clone_advice::${name}" to make it coercible`).sort(),
  )
})

it('reports suggestion severity, a declaration location, and actionable help', () => {
  expect(advice.length).toBeGreaterThan(0)
  for (const diag of advice) {
    expect(diag.severity).toBe(ServerDiagnosticSeverity.Advice)
    expect(diag.loc?.line).toBeGreaterThan(0)
    expect(diag.rendered).toContain('Clone is required for .of function')
    expect(diag.rendered).toContain('#[derive(Clone)]')
    expect(diag.rendered).toContain('implement Clone')
    expect(diag.rendered).toContain('generated coerce method')
  }
})

it('keeps bindings usable and recognizes derived and manual Clone implementations', () => {
  expect(bindings.NeedsCloneStruct).toBeDefined()
  expect(bindings.NeedsCloneStruct).not.toHaveProperty('of')
  expect(bindings.DerivedClone.of({ value: 42 }).value).toBe(42)
  expect(bindings.ManualClone.of({})).toBeInstanceOf(bindings.ManualClone)
})
