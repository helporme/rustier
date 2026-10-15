import { expect, it } from 'vitest'
import { identity_char } from './test.rs'

it('throws for a surrogate char value', () => {
  expect(() => identity_char(0xd800)).toThrow('cannot cast 55296 to char')
})

it('throws for a char value above the Unicode range', () => {
  expect(() => identity_char(0x110000)).toThrow('cannot cast 1114112 to char')
})

it.each([-1, 0x100000000, 65.5, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])(
  'throws for char input %s that cannot be represented as u32',
  value => {
    const renderedValue = value === Infinity ? 'inf' : value === -Infinity ? '-inf' : String(value)

    expect(() => identity_char(value)).toThrow(`cannot cast ${renderedValue} to u32`)
    expect(identity_char(65)).toBe(65)
  },
)
