import { expectTypeOf, test } from 'vitest'
import { Counter, Label, Point, Ready, Ticket, point_x } from './test.rs'

test('declares explicit coerce inputs separately from accessor projections', () => {
  expectTypeOf(Ready.of({})).toEqualTypeOf<Ready>()
  expectTypeOf(Point.of({ 0: 1, 1: 2 })).toEqualTypeOf<Point>()
  expectTypeOf<Label.Coerce>().toEqualTypeOf<{ text: string; position: Point.Coerce }>()
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

test('accepts nested input objects but requires complete input arguments', () => {
  // @ts-expect-error Even a zero-field coerce method requires its input object.
  Ready.of()
  // @ts-expect-error Tuple fields are required.
  Point.of({ 0: 1 })
  expectTypeOf(Label.of({ text: 'nested input', position: { 0: 1, 1: 2 } })).toEqualTypeOf<Label>()
  expectTypeOf(point_x({ 0: 1, 1: 2 })).toEqualTypeOf<number>()
})
