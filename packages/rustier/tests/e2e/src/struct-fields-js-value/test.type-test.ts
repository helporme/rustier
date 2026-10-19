import { expectTypeOf, test } from 'vitest'
import { JsMessage } from './test.rs'

test('exposes JsValue fields and their inputs as unknown', () => {
  expectTypeOf<JsMessage['payload']>().toEqualTypeOf<unknown>()
  expectTypeOf<ConstructorParameters<typeof JsMessage>>().toEqualTypeOf<
    [unknown]
  >()
  expectTypeOf<Parameters<JsMessage['replace']>>().toEqualTypeOf<[unknown]>()
  expectTypeOf<ReturnType<JsMessage['replace']>>().toEqualTypeOf<unknown>()

  const message = JsMessage.of({ payload: undefined })
  message.payload = () => 42
  expectTypeOf(message.payload).toEqualTypeOf<unknown>()
})
