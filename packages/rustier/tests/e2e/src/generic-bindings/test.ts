import { expect, it } from 'vitest'
import {
  Delivery,
  Parcel,
  Ticket,
  delivered_number,
  delivered_text,
  nested_parcel,
  number_parcel,
  read_number,
  text_parcel,
  ticket_parcel,
} from './test.rs'

it('reuses the same specialization for a generic constructor and a Rust return', () => {
  const Numbers = Parcel('i32')
  const number = number_parcel(21)
  const constructed = new Numbers(7)

  expect(Parcel('i32')).toBe(Numbers)
  expect(number).toBeInstanceOf(Numbers)
  expect(constructed.item).toBe(7)
  expect(number.doubled()).toBe(42)
  expect(read_number(constructed)).toBe(7)
})

it('substitutes generic method parameters and leaves owned clones independent', () => {
  const Strings = Parcel('String')
  const source = text_parcel('🦀')
  const duplicate = source.duplicate()

  expect(source).toBeInstanceOf(Strings)
  expect(duplicate.take()).toBe('🦀')
  expect(duplicate.item).toBe('')
  expect(source.item).toBe('🦀')
  expect(Strings.of({ item: 'hello' }).take()).toBe('hello')
})

it('uses a specialized class as the key for a nested specialization', () => {
  const Numbers = Parcel('i32')
  const Nested = Parcel(Numbers)
  const value = nested_parcel(10)
  const original = value.item

  expect(value).toBeInstanceOf(Nested)
  expect(original).toBeInstanceOf(Numbers)
  expect(Nested.of({ item: { item: 20 } }).item.item).toBe(20)
  value.item = { item: 30 }
  original.item = 99

  expect(value.item.item).toBe(30)
  expect(original.item).toBe(99)
})

it('omits methods whose bounds fail for a non-Clone, non-Default specialization', () => {
  const Tickets = Parcel(Ticket)
  const value = ticket_parcel(42)

  expect(value).toBeInstanceOf(Tickets)
  expect(value.item.number).toBe(42)
  value.item.number = 43

  expect(value.item.number).toBe(43)
  expect(value).not.toHaveProperty('duplicate')
  expect(value).not.toHaveProperty('take')
  expect(value).not.toHaveProperty('doubled')
  expect(Tickets).not.toHaveProperty('of')
  expect(Parcel('String').prototype).not.toHaveProperty('doubled')
})

it('distinguishes generic enum specializations with reversed type arguments', () => {
  const Numbers = Delivery('i32', 'String')
  const Strings = Delivery('String', 'i32')
  const number = delivered_number(42)
  const text = delivered_text('hello')
  const failed = Numbers.of({ type: 'Failed', 0: 'offline' })

  expect(number).toBeInstanceOf(Numbers)
  expect(text).toBeInstanceOf(Strings)
  expect(number).not.toBeInstanceOf(Strings)
  expect(number.type).toBe('Arrived')
  if (number.type !== 'Arrived') throw new Error('expected Delivery::Arrived')
  expect(number.item).toBe(42)
  expect(text.type).toBe('Arrived')
  if (text.type !== 'Arrived') throw new Error('expected Delivery::Arrived')
  expect(text.item).toBe('hello')
  expect(failed.type).toBe('Failed')
  if (failed.type !== 'Failed') throw new Error('expected Delivery::Failed')
  expect(failed[0]).toBe('offline')
})
