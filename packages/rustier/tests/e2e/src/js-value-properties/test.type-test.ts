import { expectTypeOf, test } from 'vitest'
import { get, make_record, set } from './test.rs'

test('keeps JS property values unknown and assignment results boolean', () => {
  expectTypeOf(get).parameters.toEqualTypeOf<[unknown, string]>()
  expectTypeOf(get).returns.toEqualTypeOf<unknown>()
  expectTypeOf(set).parameters.toEqualTypeOf<[unknown, string, unknown]>()
  expectTypeOf(set).returns.toEqualTypeOf<boolean>()
  expectTypeOf(make_record).parameters.toEqualTypeOf<[unknown]>()
  expectTypeOf(make_record).returns.toEqualTypeOf<unknown>()
})
