import { expect, it } from 'vitest'
import {
  accepts_mixed_arguments,
  add_mixed,
  choose_mixed,
  format_mixed,
} from './lib.rs'

const crab = '🦀'.codePointAt(0)!

it('accepts every supported argument by value, shared reference, and mutable reference', () => {
  expect(
    accepts_mixed_arguments(
      true,
      true,
      true,
      crab,
      crab,
      crab,
      'str',
      'mutable str',
      'owned',
      'borrowed',
      'mutable',
      -8,
      -8,
      -8,
      -16,
      -16,
      -16,
      -32,
      -32,
      -32,
      8,
      8,
      8,
      16,
      16,
      16,
      32,
      32,
      32,
      32.5,
      32.5,
      32.5,
      64.5,
      64.5,
      64.5,
    ),
  ).toBe(true)
})

it('combines a str, char, and number into a String', () => {
  expect(format_mixed('value', crab, -42)).toBe('value:🦀:-42')
})

it('combines signed, referenced, and unsigned numbers', () => {
  expect(add_mixed(-16, 32, 8)).toBe(24)
})

it('returns a char selected by a referenced bool', () => {
  expect(choose_mixed(true, crab, 'x'.codePointAt(0)!)).toBe(crab)
})
