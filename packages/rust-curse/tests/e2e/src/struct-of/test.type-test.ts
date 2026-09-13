import { expectTypeOf, test } from 'vitest'
import { Counter, Label, Point, Ready, Ticket, point_x } from './test.rs'

test('declares explicit coerce inputs separately from accessor projections', () => {
  expectTypeOf(Ready.of({})).toEqualTypeOf<Ready>()
  expectTypeOf(Point.of({ 0: 1, 1: 2 })).toEqualTypeOf<Point>()
  expectTypeOf<Label.Coerce>().toEqualTypeOf<{ text: string; position: Point }>()
  expectTypeOf<Counter.Coerce>().toEqualTypeOf<{ value: number }>()
  expectTypeOf<Counter.Plain>().toEqualTypeOf<{ value: number }>()
  expectTypeOf<Point.Plain>().toEqualTypeOf<{ 0: number; 1: number }>()
  expectTypeOf(Ticket.$of({ number: 1 })).toEqualTypeOf<Ticket>()
  expectTypeOf(Ticket.of(1)).toEqualTypeOf<Ticket>()

  // @ts-expect-error Coerce describes input data; it has no runtime value.
  Label.Coerce

  const input = { text: 'readonly input', position: Point.of({ 0: 1, 1: 2 }) } as const
  expectTypeOf(Label.of(input)).toEqualTypeOf<Label>()
})

test('requires explicit nested wrappers and a coerce input argument', () => {
  // @ts-expect-error Even a zero-field coerce method requires its input object.
  Ready.of()
  // @ts-expect-error Tuple fields are required.
  Point.of({ 0: 1 })
  // @ts-expect-error Nested objects are not implicitly constructed.
  Label.of({ text: 'not a wrapper', position: { 0: 1, 1: 2 } })
  // @ts-expect-error Ordinary function arguments are not implicitly constructed.
  point_x({ 0: 1, 1: 2 })
})
