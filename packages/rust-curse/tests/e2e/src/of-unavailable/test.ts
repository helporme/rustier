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
  expect(value.type).toBe('tuple')
  if (value.type !== 'tuple') throw new Error('expected ClosedEnum::Tuple')
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

it('coerces public fields hidden by instance methods', () => {
  expect(Shadowed.of({ value: 42 }).value()).toBe(42)
})
