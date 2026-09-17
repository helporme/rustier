import { expect, it } from 'vitest'
import { identity_i32, make_child_struct, value_struct_value } from './test.rs'

it('throws when a wrong JS type is passed to a function argument', () => {
  expect(() => identity_i32(Symbol('wrong type') as never)).toThrow(
    'Expected a value of type "number", but value of type "symbol" was passed: "Symbol(wrong type)"',
  )
})

it('throws when a number is passed where a struct is expected', () => {
  expect(() => value_struct_value(42 as never)).toThrow()
})

it('throws when a wrapper without the required value field is passed', () => {
  const child = make_child_struct(42)

  expect(() => value_struct_value(child as never)).toThrow()
})
