import { expectTypeOf, test } from 'vitest'
import { accept, choose, identity, make_object, ValueStore } from './test.rs'

test('declares owned JsValue arguments and returns as unknown', () => {
  expectTypeOf(identity).parameters.toEqualTypeOf<[unknown]>()
  expectTypeOf(identity).returns.toEqualTypeOf<unknown>()
  expectTypeOf(choose).parameters.toEqualTypeOf<[unknown, boolean, unknown]>()
  expectTypeOf(make_object).parameters.toEqualTypeOf<[]>()
  expectTypeOf(make_object).returns.toEqualTypeOf<unknown>()
  expectTypeOf(accept).returns.toEqualTypeOf<unknown>()
  expectTypeOf<ValueStore['get']>().returns.toEqualTypeOf<unknown>()
})
