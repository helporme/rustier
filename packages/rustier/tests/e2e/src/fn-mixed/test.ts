import { expect, it } from 'vitest'
import {
  accepts_mixed_arguments,
  add_mixed,
  choose_mixed,
  format_mixed,
  make_value_enum,
  make_value_struct,
} from './test.rs'

const crab = '🦀'.codePointAt(0)!

it('accepts every supported argument by value, shared reference, and mutable reference', () => {
  const structValue = make_value_struct(1)
  const structRef = make_value_struct(2)
  const structMutRef = make_value_struct(3)
  const enumValue = make_value_enum(1)
  const enumRef = make_value_enum(2)
  const enumMutRef = make_value_enum(3)

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
      structValue,
      structRef,
      structMutRef,
      enumValue,
      enumRef,
      enumMutRef,
    ),
  ).toBe(true)
  expect(structValue.value).toBe(1)
  expect(structRef.value).toBe(2)
  expect(structMutRef.value).toBe(4)

  if (enumValue.type !== 'Value') throw new Error('expected ValueEnum::Value')
  if (enumRef.type !== 'Value') throw new Error('expected ValueEnum::Value')
  if (enumMutRef.type !== 'Value') throw new Error('expected ValueEnum::Value')
  expect(enumValue.value).toBe(1)
  expect(enumRef.value).toBe(2)
  expect(enumMutRef.value).toBe(4)
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
