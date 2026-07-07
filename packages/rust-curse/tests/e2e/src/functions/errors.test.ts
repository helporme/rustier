import { describe, expect, it } from 'vitest'
import {
  identity_i16,
  identity_i32,
  identity_i8,
  identity_u16,
  identity_u32,
  identity_u8,
} from './errors.rs'

// we want to specifically check for a concrete message error to make sure that we do not
// propagate errors from the napi/wasm-bindgen
const invalidArgumentMessage = (
  paramName: string,
  fnName: string,
  rustType: string,
  jsType: string,
  reason: string,
) =>
  `Invalid argument \`${paramName}\`: expected Rust \`${rustType}\` from a JavaScript ${jsType}: ${reason}`

describe('binding errors', () => {
  it('throws a custom error when a wrong JS type is passed to a function argument', () => {
    expect(() => identity_i32(Symbol('wrong type') as never)).toThrow(
      invalidArgumentMessage(
        'value',
        'identity_i32',
        'i32',
        'number',
        'cannot cast value to i32',
      ),
    )
  })

  it('throws a custom error when a number is out of range for the Rust integer type', () => {
    const cases = [
      ['i8', identity_i8, -129, 128],
      ['i16', identity_i16, -32769, 32768],
      ['i32', identity_i32, -2147483649, 2147483648],
      ['u8', identity_u8, -1, 512],
      ['u16', identity_u16, -1, 65536],
      ['u32', identity_u32, -1, 4294967296],
    ] as const

    for (const [rustType, identity, belowMin, aboveMax] of cases) {
      for (const [name, value] of [
        ['below minimum', belowMin],
        ['above maximum', aboveMax],
      ] as const) {
        expect(() => identity(value), `${rustType} ${name}`).toThrow(
          invalidArgumentMessage(
            'value',
            `identity_${rustType}`,
            rustType,
            'number',
            `cannot cast ${value} to ${rustType}`,
          ),
        )
      }
    }
  })
})
