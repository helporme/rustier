import { expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import * as bindings from './test.rs'

// This only checks that diagnostics are emitted at all. Unit tests cover each
// individual diagnostic in detail.
const renderedDiags = rustCurseDiags.map(diag => diag.rendered).join('\n')

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
  expect(renderedDiags).toContain('rust_curse::skipped_fn')
})

it('emits an unsupported primitive diagnostic', () => {
  expect(renderedDiags).toContain('rust_curse::unsupported_primitive')
})

// TODO: Update diagnostic expectations now that Rust IR accepts type generics.
it.skip('emits an unsupported generic function diagnostic', () => {
  expect(renderedDiags).toContain('rust_curse::unsupported_generic_fn')
})

it('emits a borrowed return diagnostic', () => {
  expect(renderedDiags).toContain('rust_curse::borrowed_return')
})

it('emits an unsupported async function diagnostic', () => {
  expect(renderedDiags).toContain('rust_curse::unsupported_async_fn')
})
