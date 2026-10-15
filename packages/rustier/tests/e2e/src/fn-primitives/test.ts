import { expect, it } from 'vitest'
import {
  identity_bool,
  identity_char,
  identity_f32,
  identity_f64,
  identity_i16,
  identity_i32,
  identity_i8,
  identity_u16,
  identity_u32,
  identity_u8,
} from './test.rs'

it('preserves false', () => {
  expect(identity_bool(false)).toBe(false)
})

it('preserves true', () => {
  expect(identity_bool(true)).toBe(true)
})

it('preserves the minimum i8', () => {
  expect(identity_i8(-128)).toBe(-128)
})

it('preserves the maximum i8', () => {
  expect(identity_i8(127)).toBe(127)
})

it('preserves the minimum i16', () => {
  expect(identity_i16(-32768)).toBe(-32768)
})

it('preserves the maximum i16', () => {
  expect(identity_i16(32767)).toBe(32767)
})

it('preserves the minimum i32', () => {
  expect(identity_i32(-2147483648)).toBe(-2147483648)
})

it('preserves the maximum i32', () => {
  expect(identity_i32(2147483647)).toBe(2147483647)
})

it('preserves the minimum u8', () => {
  expect(identity_u8(0)).toBe(0)
})

it('preserves the maximum u8', () => {
  expect(identity_u8(255)).toBe(255)
})

it('preserves the minimum u16', () => {
  expect(identity_u16(0)).toBe(0)
})

it('preserves the maximum u16', () => {
  expect(identity_u16(65535)).toBe(65535)
})

it('preserves the minimum u32', () => {
  expect(identity_u32(0)).toBe(0)
})

it('preserves the maximum u32', () => {
  expect(identity_u32(4294967295)).toBe(4294967295)
})

it('preserves an ASCII char', () => {
  expect(identity_char('A'.codePointAt(0)!)).toBe('A'.codePointAt(0))
})

it('preserves a Unicode char', () => {
  expect(identity_char('🦀'.codePointAt(0)!)).toBe('🦀'.codePointAt(0))
})

it('rounds an f32 to single precision', () => {
  const value = Math.PI

  expect(identity_f32(value)).toBe(Math.fround(value))
})

it('does not preserve unsupported f32 precision', () => {
  const value = Math.PI

  expect(identity_f32(value)).not.toBe(value)
})

it('preserves the minimum finite f64', () => {
  expect(identity_f64(-Number.MAX_VALUE)).toBe(-Number.MAX_VALUE)
})

it('preserves the maximum finite f64', () => {
  expect(identity_f64(Number.MAX_VALUE)).toBe(Number.MAX_VALUE)
})

it('preserves the smallest positive f64', () => {
  expect(identity_f64(Number.MIN_VALUE)).toBe(Number.MIN_VALUE)
})

it('preserves negative infinity', () => {
  expect(identity_f64(Number.NEGATIVE_INFINITY)).toBe(Number.NEGATIVE_INFINITY)
})

it('preserves positive infinity', () => {
  expect(identity_f64(Number.POSITIVE_INFINITY)).toBe(Number.POSITIVE_INFINITY)
})

it('preserves NaN', () => {
  expect(identity_f64(Number.NaN)).toBeNaN()
})

it('preserves the sign of floating-point negative zero', () => {
  expect(identity_f64(-0)).toBe(-0)
  expect(identity_f32(-0)).toBe(-0)
})

it.each([Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.MIN_VALUE, Number.MAX_VALUE])(
  'converts f32 input %s with single-precision semantics',
  value => {
    expect(identity_f32(value)).toBe(Math.fround(value))
  },
)

it.each([0, 0xd7ff, 0xe000, 0x10ffff])('preserves Unicode scalar boundary %s', value => {
  expect(identity_char(value)).toBe(value)
})
