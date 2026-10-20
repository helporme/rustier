import { expectTypeOf, test } from 'vitest'
import { construct, define, get, has, make_symbol, own_keys } from './test.rs'

test('keeps JavaScript values unknown at the boundary', () => {
  expectTypeOf(get).parameters.toEqualTypeOf<[unknown, unknown]>()
  expectTypeOf(get).returns.toEqualTypeOf<unknown>()
  expectTypeOf(define).parameters.toEqualTypeOf<[unknown, unknown, unknown]>()
  expectTypeOf(define).returns.toEqualTypeOf<boolean>()
  expectTypeOf(has).returns.toEqualTypeOf<boolean>()
  expectTypeOf(own_keys).returns.toEqualTypeOf<unknown>()
  expectTypeOf(construct).parameters.toEqualTypeOf<[unknown, unknown]>()
  expectTypeOf(construct).returns.toEqualTypeOf<unknown>()
  expectTypeOf(make_symbol).parameters.toEqualTypeOf<[unknown]>()
})
