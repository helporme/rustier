import { expectTypeOf, test } from 'vitest'
import { none, some } from '../fn-option-js-value/test.rs'
import { OptionalMessage } from './test.rs'

test('keeps Option getters typed as wrappers with unknown payloads', () => {
  const message = new OptionalMessage(undefined)

  expectTypeOf(message.payload).toEqualTypeOf<ReturnType<typeof some>>()
  expectTypeOf(message.payload.unwrap()).toEqualTypeOf<unknown>()
  expectTypeOf(message.replace(null)).toEqualTypeOf<ReturnType<typeof some>>()

  message.payload = () => 42
  message.payload = some(undefined)
  message.payload = none()
  message.payload = null

  const coerced = OptionalMessage.of({ payload: { name: 'payload' } })
  expectTypeOf(coerced.payload).toEqualTypeOf<ReturnType<typeof some>>()
})
