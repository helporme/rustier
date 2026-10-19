import { expect, it } from 'vitest'
import { some } from '../fn-option-js-value/test.rs'
import { OptionalMessage } from './test.rs'

it('coerces JS payloads and nullish values in Option field setters', () => {
  const message = new OptionalMessage(null)
  const values: unknown[] = [false, 0, '', 42n, Symbol('payload'), {}, () => 42]

  expect(message.payload.is_none()).toBe(true)

  for (const value of values) {
    message.payload = value
    expect(message.payload.is_some()).toBe(true)
    expect(message.payload.unwrap()).toBe(value)
  }

  message.payload = null
  expect(message.payload.is_none()).toBe(true)
  message.payload = undefined
  expect(message.payload.is_none()).toBe(true)
})

it('preserves explicit Some(null) and Some(undefined) in Option fields', () => {
  const message = OptionalMessage.of({ payload: some(null) })

  expect(message.payload.is_some()).toBe(true)
  expect(message.payload.unwrap()).toBeNull()

  message.payload = some(undefined)
  expect(message.payload.is_some()).toBe(true)
  expect(message.payload.unwrap()).toBeUndefined()

  message.payload = null
  expect(message.payload.is_none()).toBe(true)
})

it('keeps a previously read Option usable after replacing the field', () => {
  const original = { name: 'original' }
  const replacement = () => 42
  const message = new OptionalMessage(original)
  const previous = message.payload

  message.payload = replacement

  expect(previous.unwrap()).toBe(original)
  expect(message.payload.unwrap()).toBe(replacement)
  expect(message.replace(null).unwrap()).toBe(replacement)
  expect(message.payload.is_none()).toBe(true)
  expect(previous.unwrap()).toBe(original)
})

it('coerces arbitrary JS object payloads during struct construction', () => {
  const payload = { type: 'Some', 0: 'ordinary data' }
  const message = OptionalMessage.of({ payload })

  expect(message.payload.unwrap()).toBe(payload)
})
