import { expect, it } from 'vitest'
import { Nested } from './nested.rs'
import { Outer } from './test.rs'

it('imports a nested class used by field accessors and Coerce inputs', () => {
  const outer = Outer.of({ nested: Nested.of({ value: 42 }), marker: 7 })

  expect(outer.nested.value).toBe(42)
  expect(outer.marker).toBe(7)
})

it('coerces nested objects and preserves instances with a method-shadowing field', () => {
  const outer = Outer.of({ nested: { value: 42 }, marker: 7 })
  expect(Outer.of(Outer.of(outer))).toBe(outer)
  expect(outer.nested.value).toBe(42)
})
