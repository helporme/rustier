import { expect, test } from 'vitest'
import { rs } from 'rustier'

const types = [
  { name: 'usize', bits: rs`usize::BITS`, signed: false, echo: (x: unknown) => rs`let x: usize = ${x}; x` },
  { name: 'u64', bits: 64, signed: false, echo: (x: unknown) => rs`let x: u64 = ${x}; x` },
  { name: 'i64', bits: 64, signed: true, echo: (x: unknown) => rs`let x: i64 = ${x}; x` },
  { name: 'u128', bits: 128, signed: false, echo: (x: unknown) => rs`let x: u128 = ${x}; x` },
  { name: 'i128', bits: 128, signed: true, echo: (x: unknown) => rs`let x: i128 = ${x}; x` },
]

for (const { name, bits, signed, echo } of types) {
  test(`${name} passes exactly as bigint within its target range`, () => {
    const limit = 1n << BigInt(signed ? bits - 1 : bits)
    const min = signed ? -limit : 0n
    const max = limit - 1n
    for (const value of [min, min + 1n, 0n, 1n, max - 1n, max, max / 3n]) {
      expect(echo(value)).toBe(value)
    }
    for (const invalid of [min - 1n, max + 1n, 1n << 256n, 1, '1', Object(1n)]) {
      expect(() => echo(invalid)).toThrow(TypeError)
    }
  })
}

test('mixes bigint and ordinary arguments without swapping or truncating words', () => {
  const high = (1n << 100n) | (1n << 63n)
  expect(rs`
    let high: u128 = ${high};
    let text: String = ${'abc'};
    let negative: i128 = ${-7n};
    let low: u64 = ${3n};
    high as i128 + negative + low as i128 + text.len() as i128
  `).toBe(high - 1n)
})

test('js! receives all five Rust types as primitive bigint values', () => {
  expect(rs`
    js!(return [@{usize::MAX}, @{i64::MIN}, @{u64::MAX}, @{i128::MIN}, @{u128::MAX}])
  `).toEqual([
    (1n << BigInt(rs`usize::BITS`)) - 1n,
    -(1n << 63n), (1n << 64n) - 1n, -(1n << 127n), (1n << 128n) - 1n,
  ])
})

test('extracts bigint results from js! without coercion or truncation', () => {
  const maxUsize = (1n << BigInt(rs`usize::BITS`)) - 1n
  expect(rs`js!(return @{i64::MIN}).unwrap().as_i64().unwrap()`).toBe(-(1n << 63n))
  expect(rs`js!(return @{u64::MAX}).unwrap().as_u64().unwrap()`).toBe((1n << 64n) - 1n)
  expect(rs`js!(return @{usize::MAX}).unwrap().as_usize().unwrap()`).toBe(maxUsize)
  expect(rs`js!(return @{usize::MAX} + 1n).unwrap().as_usize().is_none()`).toBe(true)
  expect(rs`js!(return -(1n << 63n) - 1n).unwrap().as_i64().is_none()`).toBe(true)
  expect(rs`js!(return 1n << 64n).unwrap().as_u64().is_none()`).toBe(true)
  expect(rs`js!(return -(1n << 127n)).unwrap().as_i128().unwrap()`).toBe(-(1n << 127n))
  expect(rs`js!(return (1n << 128n) - 1n).unwrap().as_u128().unwrap()`).toBe((1n << 128n) - 1n)
  expect(rs`js!(return 1n << 128n).unwrap().as_u128().is_none()`).toBe(true)
  expect(rs`js!(return 1n << 127n).unwrap().as_i128().is_none()`).toBe(true)
  expect(rs`js!(return 1n << 63n).unwrap().as_i64().is_none()`).toBe(true)
  expect(rs`js!(return -1n).unwrap().as_u64().is_none()`).toBe(true)
  expect(rs`js!(return 1).unwrap().as_usize().is_none()`).toBe(true)
})
