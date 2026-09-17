import { expectTypeOf, test } from 'vitest'
import { Nested } from './nested.rs'
import { Outer } from './test.rs'

test('imports nested classes for Coerce and exposes colliding fields in Plain', () => {
  expectTypeOf<Outer.Coerce>().toEqualTypeOf<{ nested: Nested.Coerce; marker: number }>()
  expectTypeOf<Outer.Plain>().toEqualTypeOf<{ nested: Nested; marker: number }>()
  expectTypeOf(Outer.of({ nested: Nested.of({ value: 42 }), marker: 7 })).toEqualTypeOf<Outer>()
  expectTypeOf<Outer['nested']>().toEqualTypeOf<Nested>()

  expectTypeOf(Outer.of({ nested: { value: 42 }, marker: 7 })).toEqualTypeOf<Outer>()
  expectTypeOf(Outer.of(Outer.of({ nested: { value: 42 }, marker: 7 }))).toEqualTypeOf<Outer>()
})
