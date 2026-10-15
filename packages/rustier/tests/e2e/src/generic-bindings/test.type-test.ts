import { expectTypeOf, test } from 'vitest'
import {
  Delivery,
  Parcel,
  Ticket,
  delivered_number,
  nested_parcel,
  number_parcel,
  read_number,
  text_parcel,
  ticket_parcel,
} from './test.rs'

test('substitutes primitive, nested, and class keys into generic declarations', () => {
  const Numbers = Parcel('i32')
  const Nested = Parcel(Numbers)

  expectTypeOf(new Numbers(1)).toEqualTypeOf<Parcel<'i32'>>()
  expectTypeOf(number_parcel(1).item).toEqualTypeOf<number>()
  expectTypeOf(text_parcel('hello').take()).toEqualTypeOf<string>()
  expectTypeOf(nested_parcel(1)).toEqualTypeOf<Parcel<typeof Numbers>>()
  expectTypeOf(Nested.of({ item: { item: 1 } }).item).toEqualTypeOf<Parcel<'i32'>>()
  expectTypeOf<Parcel.Plain<'i32'>>().toEqualTypeOf<{ item: number }>()
  expectTypeOf<Parcel.Coerce<'String'>>().toEqualTypeOf<{ item: string }>()
  expectTypeOf(read_number({ item: 1 })).toEqualTypeOf<number>()
})

test('omits unavailable methods and coercion from constrained specializations', () => {
  const Tickets = Parcel(Ticket)
  const value = ticket_parcel(42)

  expectTypeOf(value).toEqualTypeOf<Parcel<typeof Ticket>>()
  expectTypeOf(value.item).toEqualTypeOf<Ticket>()
  expectTypeOf(value).not.toHaveProperty('duplicate')
  expectTypeOf(value).not.toHaveProperty('take')
  expectTypeOf(value).not.toHaveProperty('doubled')
  expectTypeOf(Tickets).not.toHaveProperty('of')
  expectTypeOf<Parcel.Coerce<typeof Ticket>>().toBeNever()
  expectTypeOf(text_parcel('hello')).not.toHaveProperty('doubled')
})

test('narrows generic enum fields after substituting both type arguments', () => {
  const value = delivered_number(42)

  expectTypeOf(value).toEqualTypeOf<Delivery<'i32', 'String'>>()
  if (value.type === 'Arrived') {
    expectTypeOf(value.item).toEqualTypeOf<number>()
  } else {
    expectTypeOf(value[0]).toEqualTypeOf<string>()
  }

  expectTypeOf(Delivery('String', 'i32').of({ type: 'Failed', 0: 1 }))
    .toEqualTypeOf<Delivery<'String', 'i32'>>()
  expectTypeOf<Parcel<'u64'>>().toBeNever()
  expectTypeOf<Delivery<'bool', 'bool'>>().toBeNever()
})
