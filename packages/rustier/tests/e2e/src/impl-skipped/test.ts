import { expect, it } from 'vitest'
import { rustierDiags } from '$build/diags'
import * as bindings from './test.rs'

it('exports supported inherent methods alongside skipped items', () => {
  const widget = new bindings.Widget(42)

  expect(widget.id()).toBe(42)
})

it('does not attach trait or blanket implementations to classes', () => {
  const widget = new bindings.Widget(42) as unknown as Record<string, unknown>

  expect(widget.identity).toBeUndefined()
  expect(widget.blanket).toBeUndefined()
  expect(widget.marked).toBeUndefined()
})

it('does not export generic targets or unsupported associated items', () => {
  const runtimeBindings = bindings as unknown as Record<string, unknown>
  const runtimeClass = bindings.Widget as unknown as Record<string, unknown>
  const widget = new bindings.Widget(42) as unknown as Record<string, unknown>

  expect(runtimeBindings.Generic).toBeUndefined()
  expect(runtimeClass.DEFAULT_ID).toBeUndefined()
  expect(runtimeClass.KIND).toBeUndefined()
  expect(widget.generic).toBeUndefined()
  expect(widget.dangerous).toBeUndefined()
  expect(widget.asynchronous).toBeUndefined()
})

// TODO: Update diagnostic expectations now that Rust IR accepts type generics.
it.skip('reports unsupported methods and generic targets', () => {
  const renderedDiags = rustierDiags.map(diag => diag.rendered).join('\n')

  expect(renderedDiags).toContain('rustier::unsupported_generic_fn')
  expect(renderedDiags).toContain('rustier::unsupported_unsafe_fn')
  expect(renderedDiags).toContain('rustier::unsupported_async_fn')
})
