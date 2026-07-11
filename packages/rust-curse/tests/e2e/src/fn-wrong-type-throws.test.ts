import { expect, it } from 'vitest'
import { identity_i32 } from './lib.rs'

it('throws when a wrong JS type is passed to a function argument', () => {
  expect(() => identity_i32(Symbol('wrong type') as never)).toThrow(
    'Expected Rust `i32` from a JavaScript number: cannot cast value to i32',
  )
})
