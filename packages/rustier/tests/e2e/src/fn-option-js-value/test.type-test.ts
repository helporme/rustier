import { expectTypeOf, test } from 'vitest'
import { none, roundtrip, some } from './test.rs'

test('retains Option wrappers while exposing their JsValue payload as unknown', () => {
  const result = some(null)

  expectTypeOf(result.is_some()).toEqualTypeOf<boolean>()
  expectTypeOf(result.unwrap()).toEqualTypeOf<unknown>()
  expectTypeOf(none()).toEqualTypeOf<ReturnType<typeof some>>()
  expectTypeOf(roundtrip(result)).toEqualTypeOf<ReturnType<typeof some>>()
  expectTypeOf(roundtrip(undefined)).toEqualTypeOf<ReturnType<typeof some>>()

  if (result.type === 'Some') {
    expectTypeOf(result[0]).toEqualTypeOf<unknown>()
    result[0] = () => 42
  }
})
