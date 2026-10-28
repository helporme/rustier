import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('returns Rust booleans as JS booleans', () => {
  expect(rs`true`).toBe(true)
  expect(rs`false`).toBe(false)
  expect(rs`fn value() -> bool { true } value()`).toBe(true)
})

test.each([true, false])('passes and returns %s without JsValue conversion', value => {
  expect(rs`let value: bool = ${value}; value`).toBe(value)
  expect(rs`let value: bool = ${value}; !value`).toBe(!value)
  expect(rs`fn invert(value: bool) -> bool { !value } invert(${value})`).toBe(!value)
  expect(rs`let _: bool = ${value};`).toBeUndefined()
  expect(rs`if ${value} { true } else { false }`).toBe(value)
})

test.each([true, false])('mixes booleans, numbers, unit and JS handles for %s', flag => {
  const left = { side: 'left' }
  const right = { side: 'right' }
  const number = 42

  for (let index = 0; index < 10; index++) {
    expect(rs`
      let flag: bool = ${flag};
      let left = ${left};
      let _: () = ${undefined};
      let number: i32 = ${number};
      let other: bool = ${!flag};
      let right = ${right};
      if flag && !other && number == 42 { left } else { right }
    `).toBe(flag ? left : right)
    expect(rs`
      let left = ${left};
      let flag: bool = ${flag};
      let number: f64 = ${number};
      let right = ${right};
      flag && number == 42.0 && !left.strict_eq(&right)
    `).toBe(flag)
  }
})

test('boolean inputs reject non-booleans without coercion', () => {
  const echo = (value: unknown) => rs`let value: bool = ${value}; value`
  let coercions = 0
  const coercible = { [Symbol.toPrimitive]() { coercions++; return true } }

  for (const value of [
    undefined, null, 0, 1, -0, NaN, Infinity, '', 'true', 'false',
    0n, 1n, Symbol('bool'), {}, [], new Boolean(false), () => true, coercible,
  ]) {
    expect(() => echo(value)).toThrow(TypeError)
    expect(() => echo(value)).toThrow('Rustier input 0: expected boolean')
  }
  expect(coercions).toBe(0)
  expect(echo(false)).toBe(false)
  expect(echo(true)).toBe(true)
})

test('boolean validation precedes Rust execution', () => {
  let calls = 0
  const callback = () => { calls++ }
  const invoke = (value: unknown) => rs`
    let callback = ${callback};
    let flag: bool = ${value};
    callback.apply(&[]).unwrap();
    flag
  `

  expect(() => invoke(1)).toThrow('Rustier input 1: expected boolean')
  expect(calls).toBe(0)
  expect(invoke(false)).toBe(false)
  expect(calls).toBe(1)
})
