import { describe, expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import * as diags from './diags.rs'

// Our goal here is to check that diagnostics are emitted at all.
// We do not want to check for each individual diagnostics, it's a goal of unit tests.

describe('diagnostics', () => {
  it('exports supported functions from modules with skipped functions', () => {
    expect(diags.supported()).toBe(42)
  })

  it('does not export unsupported functions', () => {
    expect('unsupported_primitive_param' in diags).toBe(false)
    expect('generic_fn' in diags).toBe(false)
    expect('borrowed_return' in diags).toBe(false)
    expect('async_fn' in diags).toBe(false)
  })

  it('emits diagnostics for skipped functions', () => {
    const rendered = rustCurseDiags.join('\n')

    expect(rendered).toContain('rust_curse::skipped_fn')
    expect(rendered).toContain('rust_curse::unsupported_primitive')
    expect(rendered).toContain('rust_curse::unsupported_generic_fn')
    expect(rendered).toContain('rust_curse::borrowed_return')
    expect(rendered).toContain('rust_curse::unsupported_async_fn')
  })
})
