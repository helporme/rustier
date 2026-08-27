import { expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import * as bindings from './test.rs'

it('exports supported inherent methods alongside skipped items', () => {
  const widget = bindings.Widget.new(42)

  expect(widget.id()).toBe(42)
})

it('does not attach trait or blanket implementations to classes', () => {
  const widget = bindings.Widget.new(42) as unknown as Record<string, unknown>

  expect(widget.identity).toBeUndefined()
  expect(widget.blanket).toBeUndefined()
  expect(widget.marked).toBeUndefined()
})

it('does not export generic targets or unsupported associated items', () => {
  const runtimeBindings = bindings as unknown as Record<string, unknown>
  const runtimeClass = bindings.Widget as unknown as Record<string, unknown>
  const widget = bindings.Widget.new(42) as unknown as Record<string, unknown>

  expect(runtimeBindings.Generic).toBeUndefined()
  expect(runtimeClass.DEFAULT_ID).toBeUndefined()
  expect(runtimeClass.KIND).toBeUndefined()
  expect(widget.generic).toBeUndefined()
  expect(widget.dangerous).toBeUndefined()
  expect(widget.asynchronous).toBeUndefined()
})

it('reports unsupported methods and generic targets', () => {
  const renderedDiags = rustCurseDiags.join('\n')

  expect(renderedDiags).toContain('rust_curse::unsupported_generic_fn')
  expect(renderedDiags).toContain('rust_curse::unsupported_unsafe_fn')
  expect(renderedDiags).toContain('rust_curse::unsupported_async_fn')
})
