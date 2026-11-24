import { expect, test } from 'vitest'
import { rs } from 'rustier'

const record = (x: unknown) => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Record<u8>>(${x})`
const item = (x: unknown) => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Item>(${x})`
const collections = (x: unknown) => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Collections>(${x})`

test('converts private and generic fields, including magic own keys', () => {
  const input = { someValue: 3, ['__proto__']: 'plain', ignored: 42 }
  for (const opt of [{}, { optional: undefined }, { optional: null }]) {
    const out = record({ ...input, ...opt })
    expect(Object.getPrototypeOf(out)).toBe(Object.prototype)
    expect(Object.hasOwn(out, '__proto__')).toBe(true)
    expect(out).toStrictEqual({ someValue: 3, ['__proto__']: 'plain', optional: undefined })
  }
  expect(() => record(Object.create(input))).toThrow('missing required field')
  const accessor = { ...input, get someValue(): number { return this['__proto__'].length } }
  expect(record(accessor).someValue).toBe(5)
  for (const someValue of [-1, 256, 0.5, Infinity, NaN, '3']) {
    expect(() => record({ ...input, someValue })).toThrow('expected u8')
  }
})

test('converts tagged and unit enums, preserving tuple variant arity', () => {
  for (const x of [{ kind: 'Empty' }, { kind: 'Named', count: 4 }, { kind: 'Pair', values: ['pear!', -4] }]) {
    expect(item(x)).toStrictEqual(x)
  }
  expect(() => item({ kind: 'Pair', values: ['green_apple'] })).toThrow('length')
  expect(() => item({ kind: 'Pair', values: ['Cherry', 3] })).toThrow('unknown Fruit variant')
})

test('converts generic enum payloads using the concrete type argument', () => {
  const bytes = (x: unknown) => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Batch<u8>>(${x})`
  const fruits = (x: unknown) => rs`use handy_crate::conversions::{round_trip, Batch, Fruit}; round_trip::<Batch<Fruit>>(${x})`
  const one = { type: 'One', values: [255] }
  const many = { type: 'Many', items: ['green_apple', 'pear!'] }

  expect(bytes({ type: 'Empty' })).toStrictEqual({ type: 'Empty' })
  expect(bytes(one)).toStrictEqual(one)
  expect(fruits(many)).toStrictEqual(many)
  expect(() => bytes(many)).toThrow('expected u8')
})

test('recursively converts generic structs', () => {
  const tree = { value: 1, children: [{ value: 2, children: [] }] }
  expect(rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Tree<u32>>(${tree})`).toStrictEqual(tree)
})

test('handle fields wrap even strings and move the original value exactly once', () => {
  const ticket = () => rs`handy_crate::conversions::Ticket { payload: String::from("Fluffy"), count: 1 }.to_js(__rustier_env)`
  const read = (value: unknown) => rs`handy_crate::conversions::Ticket::<String>::from_js(${value}).map(|t| t.payload)`
  const value = ticket()
  expect(rs`let value: JsValue = ${value.token}; value.is_rust_handle::<String>()`).toBe(true)
  expect(read(value)).toBe('Fluffy')
  expect(() => read(value)).toThrow('moved')
  expect(() => read({ token: 'Fluffy', count: 1 })).toThrow('expected Rust handle')
  expect(() => read({ count: 1 })).toThrow('missing required field')
  const invalid = { ...ticket(), count: -1 }
  expect(() => read(invalid)).toThrow('expected u8')
  expect(() => read(invalid)).toThrow('moved')
})

test('tuple variants accept ordinary Rust handles without conversion traits', () => {
  const handle = rs`handy_crate::CopyValue(42)`
  const value = { type: 'Store', values: [handle] }
  const convert = (input: unknown) => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Action<handy_crate::CopyValue>>(${input})`
  expect(() => convert({ ...value, values: [rs`vec![42u32]`] })).toThrow('expected Rust handle')
  const out = convert(value)
  expect(out.type).toBe('Store')
  expect(rs`let value: handy_crate::CopyValue = ${out.values[0]}; value.0`).toBe(42)
  expect(() => convert(value)).toThrow('moved')
})

test('round-trips wide integers without passing through JS number', () => {
  const convert = (x: unknown) => rs`handy_crate::conversions::round_trip::<(i64, u64, i128, u128, isize, usize)>(${x})`
  const bits = BigInt(rs`usize::BITS`)
  const min = [-(1n << 63n), 0n, -(1n << 127n), 0n, -(1n << (bits - 1n)), 0n]
  const max = [(1n << 63n) - 1n, (1n << 64n) - 1n, (1n << 127n) - 1n, (1n << 128n) - 1n, (1n << (bits - 1n)) - 1n, (1n << bits) - 1n]
  expect(convert(min)).toStrictEqual(min)
  expect(convert(max)).toStrictEqual(max)
  for (const [i, value] of max.entries()) {
    for (const invalid of [min[i] - 1n, value + 1n, 0]) {
      const input: unknown[] = [...max]
      input[i] = invalid
      expect(() => convert(input)).toThrow('expected')
    }
  }
})

test('unit reads null and undefined, and writes undefined', () => {
  const unit = (x: unknown) => rs`handy_crate::conversions::round_trip::<()>(${x})`
  expect(unit(null)).toBeUndefined()
  expect(unit(undefined)).toBeUndefined()
  expect(() => unit(false)).toThrow('expected null or undefined')
})

test('uses native JS collections and selects map shape even for empty maps', () => {
  const input = {
    array: [1, -2], tuple: [1.5, true], vec: ['a', '🌱'], set: new Set([1, 2]),
    ordered_set: new Set(['a']), record: { ['__proto__']: { kind: 'Empty' } }, map: new Map([[5, 7]]),
  }
  expect(collections(input)).toStrictEqual(input)
  expect(collections({ ...input, record: {}, map: new Map() }).map).toBeInstanceOf(Map)
  expect(() => collections({ ...input, array: [1] })).toThrow('length')
  expect(() => collections({ ...input, tuple: [1, true, 3] })).toThrow('length')
  expect(() => collections({ ...input, set: [] })).toThrow()
  expect(() => collections({ ...input, map: {} })).toThrow()
})

test('selects map representation by Rust key type, not by entries', () => {
  const strings = (x: unknown) => rs`handy_crate::conversions::round_trip::<std::collections::BTreeMap<String, u8>>(${x})`
  const numbers = (x: unknown) => rs`handy_crate::conversions::round_trip::<std::collections::HashMap<u8, String>>(${x})`
  expect(strings({})).toStrictEqual({})
  expect(strings({ ['__proto__']: 42 })).toStrictEqual({ ['__proto__']: 42 })
  expect(numbers(new Map())).toStrictEqual(new Map())
  expect(numbers(new Map([[1, 'one']]))).toStrictEqual(new Map([[1, 'one']]))
})

test('a generic field is optional only when instantiated with Option', () => {
  const input = { ['__proto__']: 'own' }
  expect(rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Record<Option<u8>>>(${input})`).toStrictEqual({
    someValue: undefined, ['__proto__']: 'own', optional: undefined,
  })
  expect(() => rs`handy_crate::conversions::round_trip::<handy_crate::conversions::Record<JsValue>>(${input})`).toThrow('missing required field')
})
