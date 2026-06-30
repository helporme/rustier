import { describe, expect, it } from 'vitest'
import {
  accepts_all_primitives,
  accepts_refs,
  identity_bool,
  identity_f64,
  identity_i16,
  identity_i32,
  identity_i8,
  identity_u16,
  identity_u32,
  identity_u8,
  no_args_no_return,
  return_bool,
  return_f64,
  return_i16,
  return_i32,
  return_i8,
  return_u16,
  return_u32,
  return_u8,
  str_eq,
} from './primitive.rs'

describe('primitive functions', () => {
  it('accepts all supported primitive parameters', () => {
    expect(
      accepts_all_primitives(true, 'rust', -8, -16, -32, 8, 16, 32, 64.5),
    ).toBe(true)
  })

  it('accepts references to supported primitive parameters', () => {
    expect(accepts_refs(true, 'curse', 42, 1.5)).toBe(true)
  })

  it('preserves primitive edge values', () => {
    expect(identity_bool(false)).toBe(false)
    expect(identity_bool(true)).toBe(true)

    expect(identity_i8(-128)).toBe(-128)
    expect(identity_i8(127)).toBe(127)
    expect(identity_i16(-32768)).toBe(-32768)
    expect(identity_i16(32767)).toBe(32767)
    expect(identity_i32(-2147483648)).toBe(-2147483648)
    expect(identity_i32(2147483647)).toBe(2147483647)

    expect(identity_u8(0)).toBe(0)
    expect(identity_u8(255)).toBe(255)
    expect(identity_u16(0)).toBe(0)
    expect(identity_u16(65535)).toBe(65535)
    expect(identity_u32(0)).toBe(0)
    expect(identity_u32(4294967295)).toBe(4294967295)

    expect(identity_f64(-Number.MAX_VALUE)).toBe(-Number.MAX_VALUE)
    expect(identity_f64(Number.MAX_VALUE)).toBe(Number.MAX_VALUE)
    expect(identity_f64(Number.MIN_VALUE)).toBe(Number.MIN_VALUE)
    expect(identity_f64(Number.NEGATIVE_INFINITY)).toBe(
      Number.NEGATIVE_INFINITY,
    )
    expect(identity_f64(Number.POSITIVE_INFINITY)).toBe(
      Number.POSITIVE_INFINITY,
    )
    expect(identity_f64(Number.NaN)).toBeNaN()
  })

  it('passes strings with different UTF characters', () => {
    const values = [
      'ascii',
      'café',
      'Здравствуйте',
      'こんにちは',
      'مرحبا',
      '🦀✨',
      'á',
    ]

    for (const value of values) {
      expect(str_eq(value, value)).toBe(true)
    }
  })

  it('returns supported primitive values', () => {
    expect(return_bool()).toBe(true)
    expect(return_i8()).toBe(-8)
    expect(return_i16()).toBe(-16)
    expect(return_i32()).toBe(-32)
    expect(return_u8()).toBe(8)
    expect(return_u16()).toBe(16)
    expect(return_u32()).toBe(32)
    expect(return_f64()).toBe(64.5)
  })

  it('calls a function without arguments or return value', () => {
    expect(no_args_no_return()).toBeUndefined()
  })
})
