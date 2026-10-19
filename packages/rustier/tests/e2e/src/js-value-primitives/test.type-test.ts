import { expectTypeOf, test } from 'vitest'
import {
  make_boolean,
  make_null,
  make_string,
  make_undefined,
  read_boolean,
  read_number,
  read_string,
  type_name,
} from './test.rs'

test('keeps inspected values unknown and extracted values typed', () => {
  expectTypeOf(type_name).parameters.toEqualTypeOf<[unknown]>()
  expectTypeOf(type_name).returns.toEqualTypeOf<string>()
  expectTypeOf(read_number(1).unwrap()).toEqualTypeOf<number>()
  expectTypeOf(read_boolean(false).unwrap()).toEqualTypeOf<boolean>()
  expectTypeOf(read_string('').unwrap()).toEqualTypeOf<string>()
})

test('omits injected environments from primitive constructors', () => {
  expectTypeOf(make_string).parameters.toEqualTypeOf<[string]>()
  expectTypeOf(make_string).returns.toEqualTypeOf<unknown>()
  expectTypeOf(make_boolean).parameters.toEqualTypeOf<[boolean]>()
  expectTypeOf(make_boolean).returns.toEqualTypeOf<unknown>()
  expectTypeOf(make_null).parameters.toEqualTypeOf<[]>()
  expectTypeOf(make_undefined).parameters.toEqualTypeOf<[]>()
})
