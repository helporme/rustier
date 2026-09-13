import { expectTypeOf, test } from 'vitest'
import { Nested } from './nested.rs'
import { Outer } from './test.rs'

test('imports Coerce-only classes without exposing hidden fields in Plain', () => {
  expectTypeOf<Outer.Coerce>().toEqualTypeOf<{ nested: Nested; marker: number }>()
  expectTypeOf<Outer.Plain>().toEqualTypeOf<{ marker: number }>()
  expectTypeOf(Outer.of({ nested: Nested.of({ value: 42 }), marker: 7 })).toEqualTypeOf<Outer>()
  expectTypeOf<Outer['nested']>().toEqualTypeOf<() => number>()

  // @ts-expect-error Coerce requires an explicit Nested wrapper.
  Outer.of({ nested: { value: 42 }, marker: 7 })
})
