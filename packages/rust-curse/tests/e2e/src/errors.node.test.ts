import { describe, expect, it } from 'vitest'
import { identity_i32 } from './errors.rs'

describe('Node binding errors', () => {
  it('throws when a wrong JS type is passed to a function argument', () => {
    expect(() => identity_i32(Symbol('wrong type') as never)).toThrow(
      'Failed to convert napi value Symbol into rust type `i32`',
    )
  })
})
