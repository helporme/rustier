import { expect, it } from 'vitest'
import { rustierDiags } from '$build/diags'
import * as bindings from './test.rs'

// This only checks that diagnostics are emitted at all. Unit tests cover each
// individual diagnostic in detail.
const renderedDiags = rustierDiags.map(diag => diag.rendered).join('\n')

it('exports supported functions alongside skipped functions', () => {
  expect(bindings.supported()).toBe(42)
})

it('does not export functions with unsupported primitive parameters', () => {
  expect('unsupported_primitive_param' in bindings).toBe(false)
})

it('does not export generic functions', () => {
  expect('generic_fn' in bindings).toBe(false)
})

it('does not export functions with borrowed returns', () => {
  expect('borrowed_return' in bindings).toBe(false)
})

it('does not export async functions', () => {
  expect('async_fn' in bindings).toBe(false)
})

it('emits a skipped function diagnostic', () => {
  expect(renderedDiags).toContain('rustier::skipped_fn')
})

it('emits an unsupported primitive diagnostic', () => {
  expect(renderedDiags).toContain('rustier::unsupported_primitive')
})

it('does not diagnose type-generic functions as unsupported declarations', () => {
  expect(renderedDiags).not.toContain('rustier::unsupported_generic_fn')
})

it('emits a borrowed return diagnostic', () => {
  expect(renderedDiags).toContain('rustier::borrowed_return')
})

it('emits an unsupported async function diagnostic', () => {
  expect(renderedDiags).toContain('rustier::unsupported_async_fn')
})
