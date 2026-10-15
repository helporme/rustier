import { expect, it } from 'vitest'
import {
  closed_tuple,
  ClosedEnum,
  NotClone,
  NotCloneEnum,
  OpenRecord,
  Owner,
  PrivateField,
  Shadowed,
  UnsupportedField,
} from './test.rs'

it('reads non-exhaustive variants returned by Rust without exposing coerce', () => {
  const value = closed_tuple(42)
  expect(value.type).toBe('Tuple')
  if (value.type !== 'Tuple') throw new Error('expected ClosedEnum::Tuple')
  expect(value[0]).toBe(42)
})

it('does not attach coerce methods to ineligible classes', () => {
  for (const cls of [
    ClosedEnum,
    NotClone,
    NotCloneEnum,
    OpenRecord,
    Owner,
    PrivateField,
    UnsupportedField,
  ]) {
    expect(cls).not.toHaveProperty('of')
    expect(cls).not.toHaveProperty('$of')
  }
})

it('coerces public fields that shadow instance methods', () => {
  const shadowed = Shadowed.of({ value: 42 })
  expect(Shadowed.of(Shadowed.of(shadowed))).toBe(shadowed)
  expect(shadowed.value).toBe(42)
  shadowed.value = 43
  expect(shadowed.value).toBe(43)
})
