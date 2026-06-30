import { describe, expect, it } from 'vitest'
import { identity_i32 } from './errors.rs'

describe('wasm binding errors', () => {
  it('throws when a wrong JS type is passed to a function argument', () => {
    expect(() => identity_i32(Symbol('wrong type') as never)).toThrow(
      'Cannot convert a Symbol value to a number',
    )
  })
})
