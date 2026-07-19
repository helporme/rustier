import { expect, it } from 'vitest'
import {
  identity_i16,
  identity_i32,
  identity_i8,
  identity_u16,
  identity_u32,
  identity_u8,
} from './test.rs'

const invalidArgumentMessage = (
  rustType: string,
  jsType: string,
  reason: string,
) => `Expected Rust \`${rustType}\` from a JavaScript ${jsType}: ${reason}`

const cases = [
  ['i8', identity_i8, -129, 128],
  ['i16', identity_i16, -32769, 32768],
  ['i32', identity_i32, -2147483649, 2147483648],
  ['u8', identity_u8, -1, 512],
  ['u16', identity_u16, -1, 65536],
  ['u32', identity_u32, -1, 4294967296],
] as const

for (const [rustType, identity, belowMin, aboveMax] of cases) {
  it.each([
    ['below minimum', belowMin],
    ['above maximum', aboveMax],
  ] as const)(`throws for ${rustType} %s`, (_name, value) => {
    expect(() => identity(value)).toThrow(
      invalidArgumentMessage(
        rustType,
        'number',
        `cannot cast ${value} to ${rustType}`,
      ),
    )
  })
}
