import { expect, it } from 'vitest'
import { identity_char } from './lib.rs'

it('throws for a surrogate char value', () => {
  expect(() => identity_char(0xd800)).toThrow('cannot cast 55296 to char')
})

it('throws for a char value above the Unicode range', () => {
  expect(() => identity_char(0x110000)).toThrow('cannot cast 1114112 to char')
})

it('throws for a char value outside the u32 range', () => {
  expect(() => identity_char(0x1_0000_0000)).toThrow(
    'cannot cast value to char',
  )
})
