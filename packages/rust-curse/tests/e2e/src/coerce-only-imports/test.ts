import { expect, it } from 'vitest'
import { Nested } from './nested.rs'
import { Outer } from './test.rs'

it('imports a nested class referenced only by the outer Coerce input', () => {
  const outer = Outer.of({ nested: Nested.of({ value: 42 }), marker: 7 })

  expect(outer.nested()).toBe(42)
  expect(outer.marker).toBe(7)
})
