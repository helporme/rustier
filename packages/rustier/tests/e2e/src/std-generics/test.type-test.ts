import { expectTypeOf, test } from 'vitest'
import { Vec } from '$rust/alloc/vec'
import { Option } from '$rust/core/option'
import { Result } from '$rust/core/result'
import { checked_number, nested_words, optional_number, words } from './test.rs'

test('preserves standard-library specialization identity across virtual module imports', () => {
  const Strings = Vec('String')

  expectTypeOf(words()).toEqualTypeOf<Vec<'String'>>()
  expectTypeOf(new Strings()).toEqualTypeOf<Vec<'String'>>()
  expectTypeOf(nested_words()).toEqualTypeOf<Vec<typeof Strings>>()
  expectTypeOf(words().pop()).toEqualTypeOf<Option<'String'>>()
  expectTypeOf(optional_number(true)).toEqualTypeOf<Option<'i32'>>()
  expectTypeOf(checked_number(false)).toEqualTypeOf<Result<'i32', 'String'>>()
})
