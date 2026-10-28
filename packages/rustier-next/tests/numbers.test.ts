import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

const integers = [
  { name: 'i8', min: -128, max: 127, echo: (value: number) => rs`let value: i8 = ${value}; value` },
  { name: 'i16', min: -32768, max: 32767, echo: (value: number) => rs`let value: i16 = ${value}; value` },
  { name: 'i32', min: -2147483648, max: 2147483647, echo: (value: number) => rs`let value: i32 = ${value}; value` },
  { name: 'u8', min: 0, max: 255, echo: (value: number) => rs`let value: u8 = ${value}; value` },
  { name: 'u16', min: 0, max: 65535, echo: (value: number) => rs`let value: u16 = ${value}; value` },
  { name: 'u32', min: 0, max: 4294967295, echo: (value: number) => rs`let value: u32 = ${value}; value` },
]

for (const { name, min, max, echo } of integers) {
  test(`${name} inputs and outputs saturate at their bounds`, () => {
    for (const value of [0, 1, 42, min, min + 1, max - 1, max]) {
      expect(echo(value)).toBe(value)
    }
    for (const value of [min - 0.5, min - 1, -Number.MAX_VALUE, -Infinity]) {
      expect(echo(value)).toBe(min)
    }
    for (const value of [max + 0.5, max + 1, Number.MAX_VALUE, Infinity]) {
      expect(echo(value)).toBe(max)
    }
    expect(echo(12.9)).toBe(12)
    expect(echo(-12.9)).toBe(min < 0 ? -12 : 0)
    expect(echo(max - 0.5)).toBe(max - 1)
    expect(echo(min + 0.5)).toBe(min < 0 ? min + 1 : 0)
    for (const value of [0.5, -0.5, Number.MIN_VALUE, -Number.MIN_VALUE]) {
      expect(echo(value)).toBe(0)
    }
    expect(echo(NaN)).toBe(0)
    expect(echo(-0)).toBe(0)
  })
}

test('f32 rounds and clamps inputs', () => {
  const echo = (value: number) => rs`let value: f32 = ${value}; value`
  const max = (2 - 2 ** -23) * 2 ** 127

  for (const value of [
    0, -0, 1.5, -1.5, 1 / 3,
    1 + 2 ** -24, 1 + 3 * 2 ** -24,
    2 ** 24 + 1, 2 ** 24 + 3,
    2 ** -126, 2 ** -149, 2 ** -150, -(2 ** -150),
    3 * 2 ** -150, -3 * 2 ** -150,
    Number.MIN_VALUE, -Number.MIN_VALUE, max, -max, NaN,
  ]) {
    expect(echo(value)).toBe(Math.fround(value))
  }
  for (const value of [max + 2 ** 103, 2 ** 128, Number.MAX_VALUE, Infinity]) {
    expect(echo(value)).toBe(max)
    expect(echo(-value)).toBe(-max)
  }
})

test('f64 preserves JavaScript numbers', () => {
  const echo = (value: number) => rs`let value: f64 = ${value}; value`

  for (const value of [
    0, -0, 1.5, -1.5, 1 / 3,
    Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1,
    Number.MIN_VALUE, -Number.MIN_VALUE, Number.MAX_VALUE, -Number.MAX_VALUE,
    Infinity, -Infinity, NaN,
  ]) {
    expect(echo(value)).toBe(value)
  }
})

test('numeric outputs', () => {
  expect(rs`-128i8`).toBe(-128)
  expect(rs`-32768i16`).toBe(-32768)
  expect(rs`-2147483648i32`).toBe(-2147483648)
  expect(rs`255u8`).toBe(255)
  expect(rs`65535u16`).toBe(65535)
  expect(rs`4294967295u32`).toBe(4294967295)
  expect(rs`1.25f32`).toBe(1.25)
  expect(rs`1.25f64`).toBe(1.25)
  expect(rs`42`).toBe(42)
  expect(rs`1.5`).toBe(1.5)
})

test('Rust-produced floating point outputs preserve special values', () => {
  expect(rs`f32::INFINITY`).toBe(Infinity)
  expect(rs`f32::NEG_INFINITY`).toBe(-Infinity)
  expect(rs`f32::NAN`).toBeNaN()
  expect(rs`-0.0f32`).toBe(-0)
  expect(rs`f32::MAX`).toBe((2 - 2 ** -23) * 2 ** 127)
  expect(rs`f32::MIN_POSITIVE`).toBe(2 ** -126)
  expect(rs`f32::from_bits(1)`).toBe(2 ** -149)
  expect(rs`1.0f32 / 3.0`).toBe(Math.fround(1 / 3))
  expect(rs`f64::INFINITY`).toBe(Infinity)
  expect(rs`f64::NEG_INFINITY`).toBe(-Infinity)
  expect(rs`f64::NAN`).toBeNaN()
  expect(rs`-0.0f64`).toBe(-0)
  expect(rs`f64::MAX`).toBe(Number.MAX_VALUE)
  expect(rs`f64::MIN`).toBe(-Number.MAX_VALUE)
  expect(rs`f64::from_bits(1)`).toBe(Number.MIN_VALUE)
})

test('numeric inputs can return unit', () => {
  const value = 42

  expect(rs`let value: i8 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: i16 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: i32 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: u8 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: u16 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: u32 = ${value}; assert_eq!(value, 42);`).toBeUndefined()
  expect(rs`let value: f32 = ${value}; assert_eq!(value, 42.0);`).toBeUndefined()
  expect(rs`let value: f64 = ${value}; assert_eq!(value, 42.0);`).toBeUndefined()
})

test('numeric inputs are inferred through function calls and aliases', () => {
  const value = 300

  expect(rs`
    type Number = u8;
    fn consume(value: Number) -> i16 { value as i16 - 1 }
    consume(${value})
  `).toBe(254)
})

test('numeric inputs reject non-numbers without coercing them', () => {
  const echoes = [
    ...integers.map(({ echo }) => echo),
    (value: unknown) => rs`let value: f32 = ${value}; value`,
    (value: unknown) => rs`let value: f64 = ${value}; value`,
  ]
  let coercions = 0
  const coercible = {
    [Symbol.toPrimitive]() {
      coercions++
      return 42
    },
  }
  const invalid: any[] = [
    undefined, null, true, false, '42', '', 42n, Symbol('number'),
    {}, [], [42], new Number(42), () => 42, coercible,
  ]

  for (const echo of echoes) {
    for (const value of invalid) {
      expect(() => echo(value)).toThrow(TypeError)
      expect(() => echo(value)).toThrow('Rustier input 0: expected number')
    }
    expect(echo(42)).toBe(42)
  }
  expect(coercions).toBe(0)
})

test('unit inputs require undefined and JsValue inputs remain unrestricted', () => {
  const unit = (value: unknown) => rs`let _: () = ${value};`
  const echo = (value: unknown) => rs`${value}`

  expect(unit(undefined)).toBeUndefined()
  for (const value of [null, 0, false, '', {}, [], 1n, Symbol('value')]) {
    expect(() => unit(value)).toThrow(TypeError)
    expect(() => unit(value)).toThrow('Rustier input 0: expected undefined')
    expect(echo(value)).toBe(value)
  }
})

test('all inputs are validated before the Rust body runs', () => {
  let calls = 0
  const callback = () => { calls++ }
  const invoke = (value: unknown) => rs`
    let callback = ${callback};
    let value: i32 = ${value};
    callback.apply(&[]).unwrap();
    value
  `

  expect(() => invoke('42')).toThrow('Rustier input 1: expected number')
  expect(calls).toBe(0)
  expect(invoke(42)).toBe(42)
  expect(calls).toBe(1)
})

test('numbers, handles, and unit inputs retain their positions', () => {
  const left = 12
  const right = 30
  const object = { value: 7 }
  const unit = undefined

  expect(rs`
    let left: i16 = ${left};
    let object = ${object};
    let _: () = ${unit};
    let right: f64 = ${right};
    let other = ${object};
    if object.strict_eq(&other) { left as f64 + right } else { 0.0 }
  `).toBe(42)
  expect(rs`
    let _: u8 = ${left};
    let object = ${object};
    let _: f32 = ${right};
    object
  `).toBe(object)
})
