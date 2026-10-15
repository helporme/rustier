import { expect, it } from 'vitest'
import { Delivery, Parcel, read_number, text_parcel } from '../generic-bindings/test.rs'

it.each([
  [],
  ['u64'],
  ['i32', 'String'],
  [Number],
  [{}],
])('rejects unavailable Parcel specialization %j', (...args) => {
  expect(() => (Parcel as Function)(...args)).toThrow('Unsupported generic arguments for Parcel')
  expect(Parcel('i32').of({ item: 42 }).item).toBe(42)
})

it('rejects a wrong generic specialization without consuming it', () => {
  const text = text_parcel('still alive')

  expect(() => read_number(text as never)).toThrow()
  expect(text.item).toBe('still alive')
  expect(read_number({ item: 42 })).toBe(42)
})

it('does not confuse a generic template with a specialized class key', () => {
  expect(() => (Parcel as Function)(Parcel)).toThrow('Unsupported generic arguments for Parcel')
  expect(() => (Delivery as Function)('i32')).toThrow('Unsupported generic arguments for Delivery')
  expect(() => (Delivery as Function)('i32', 'i32')).toThrow('Unsupported generic arguments for Delivery')
})
