import { expect, it } from 'vitest'
import { JsMessage } from './test.rs'

const values: unknown[] = [
  undefined,
  null,
  false,
  0,
  -0,
  NaN,
  '',
  12345678901234567890n,
  Symbol('payload'),
  { name: 'payload' },
  [1, 2],
  () => 42,
]

it('reads and assigns every JS value kind through a struct field', () => {
  const message = new JsMessage('initial')

  for (const value of values) {
    message.payload = value
    expect(message.payload).toBe(value)
    expect(message.payload).toBe(value)
  }
})

it('coerces object literals without wrapping or copying their JS payloads', () => {
  for (const value of values) {
    expect(JsMessage.of({ payload: value }).payload).toBe(value)
  }
})

it('keeps previously read values usable after replacing the Rust field', () => {
  const original = { name: 'original' }
  const replacement = () => 42
  const message = new JsMessage(original)
  const read = message.payload

  expect(message.replace(replacement)).toBe(original)
  expect(read).toBe(original)
  expect(message.payload).toBe(replacement)

  message.payload = undefined
  expect(message.payload).toBeUndefined()
  expect(read).toBe(original)
})
