import { expect, it } from 'vitest'
import { none, roundtrip, some, unwrap_or } from './test.rs'

it('returns Some wrappers with unchanged JS payloads, including null and undefined', () => {
  const values: unknown[] = [
    undefined,
    null,
    false,
    0,
    '',
    42n,
    Symbol('payload'),
    { name: 'payload' },
    () => 42,
  ]

  for (const value of values) {
    const result = some(value)

    expect(result.type).toBe('Some')
    expect(result.is_some()).toBe(true)
    expect(result.unwrap()).toBe(value)
    expect(roundtrip(result).unwrap()).toBe(value)

    if (result.type !== 'Some') throw new Error('expected Some')
    expect(result[0]).toBe(value)
  }
})

it('distinguishes None from Some(null) and Some(undefined)', () => {
  expect(none().type).toBe('None')
  expect(none().is_none()).toBe(true)
  expect(roundtrip(null).is_none()).toBe(true)
  expect(roundtrip(undefined).is_none()).toBe(true)
  expect(roundtrip(some(null)).is_some()).toBe(true)
  expect(roundtrip(some(null)).unwrap()).toBeNull()
  expect(roundtrip(some(undefined)).is_some()).toBe(true)
  expect(roundtrip(some(undefined)).unwrap()).toBeUndefined()

  const fallback = { name: 'fallback' }
  expect(unwrap_or(none(), fallback)).toBe(fallback)
  expect(unwrap_or(some(null), fallback)).toBeNull()
  expect(unwrap_or(some(undefined), fallback)).toBeUndefined()
})

it('treats ordinary objects with Option-like tags as JS payloads', () => {
  for (const value of [{ type: 'Some', 0: 42 }, { type: 'None' }]) {
    expect(roundtrip(value).is_some()).toBe(true)
    expect(roundtrip(value).unwrap()).toBe(value)
  }
})

it('clones the Option value without copying the JS payload', () => {
  const original = { name: 'original' }
  const replacement = () => 42
  const first = some(original)
  const copy = roundtrip(first)

  if (first.type !== 'Some') throw new Error('expected Some')
  first[0] = replacement

  expect(first.unwrap()).toBe(replacement)
  expect(copy.unwrap()).toBe(original)
})
