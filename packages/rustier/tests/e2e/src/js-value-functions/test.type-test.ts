import { expectTypeOf, test } from 'vitest'
import {
  apply0,
  apply2,
  apply_as0,
  apply_as2,
  apply_many,
  call0,
  call2,
} from './test.rs'

test('uses unknown for functions, receivers, arguments, and returned JS values', () => {
  expectTypeOf(apply0).parameters.toEqualTypeOf<[unknown]>()
  expectTypeOf(apply2).parameters.toEqualTypeOf<[unknown, unknown, unknown]>()
  expectTypeOf(apply_as0).parameters.toEqualTypeOf<[unknown, unknown]>()
  expectTypeOf(apply_as2).parameters.toEqualTypeOf<
    [unknown, unknown, unknown, unknown]
  >()
  expectTypeOf(apply_many).parameters.toEqualTypeOf<[unknown, number]>()
  expectTypeOf(call0).parameters.toEqualTypeOf<[unknown, string]>()
  expectTypeOf(call2).parameters.toEqualTypeOf<
    [unknown, string, unknown, unknown]
  >()
  expectTypeOf(apply0).returns.toEqualTypeOf<unknown>()
  expectTypeOf(call0).returns.toEqualTypeOf<unknown>()
})
