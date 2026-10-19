import { expect, it } from 'vitest'
import {
  accept,
  checked,
  choose,
  identity,
  make_number,
  make_object,
  reject,
  ValueStore,
} from './test.rs'

const values: [string, unknown][] = [
  ['undefined', undefined],
  ['null', null],
  ['boolean', true],
  ['number', 42],
  ['NaN', NaN],
  ['negative zero', -0],
  ['string', 'hello 🦀'],
  ['bigint', 12345678901234567890n],
  ['symbol', Symbol('value')],
  ['object', { value: 42 }],
  ['array', [1, 2, 3]],
  ['function', (value: number) => value + 1],
]

it.each(values)('round-trips a %s through an owned JsValue', (_name, value) => {
  expect(identity(value)).toBe(value)
})

it.each(values)('preserves a %s returned as a Result error', (_name, value) => {
  let caught = false

  try {
    reject(value)
  } catch (error) {
    caught = true
    expect(error).toBe(value)
  }

  expect(caught).toBe(true)
})

it('preserves JsValue arguments around an injected environment and a boolean', () => {
  const first = { name: 'first' }
  const second = () => 42

  expect(choose.length).toBe(3)
  expect(choose(first, true, second)).toBe(first)
  expect(choose(first, false, second)).toBe(second)
})

it('returns actual JavaScript values created by Rust', () => {
  expect(make_number(42.5)).toBe(42.5)
  expect(make_number(-0)).toBe(-0)
  expect(make_object()).toEqual({})
  expect(make_object()).not.toBe(make_object())
})

it('retains the original object in Rust and returns independent owning clones', () => {
  const value = { message: 'before' }
  const store = new ValueStore(value)

  value.message = 'after'

  expect(store.get()).toBe(value)
  expect((store.get() as typeof value).message).toBe('after')
  expect(store.get()).toBe(value)

  const copy = new ValueStore(store.get())

  store.clear()
  expect(copy.get()).toBe(value)
  expect(copy.take()).toBe(value)
})

it('returns an owned JsValue through a successful Result', () => {
  const value = () => 42

  expect(accept(value)).toBe(value)
})

it('can pass values again after a later Rust argument conversion fails', () => {
  const value = { value: 42 }

  expect(() => checked(value, 300)).toThrow()
  expect(checked(value, 1)).toBe(value)
  expect(identity(value)).toBe(value)
})

it('does not access properties or coerce objects while crossing the boundary', () => {
  const value = new Proxy(
    {},
    {
      get() {
        throw new Error('unexpected property access')
      },
    },
  )

  expect(identity(value)).toBe(value)
})
