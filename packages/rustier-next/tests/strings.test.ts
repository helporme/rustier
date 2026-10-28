import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('returns owned strings without JsValue conversion', () => {
  expect(rs`String::new()`).toBe('')
  expect(rs`"héllo\0🦀".to_owned()`).toBe('héllo\0🦀')
  expect(rs`format!("{} {}", "hello", "world")`).toBe('hello world')
})

test('accepts and returns owned strings', () => {
  for (const value of ['', 'hello', 'héllo\0🦀', '你好', '\0\0', 'e\u0301', '\uFEFFtext', '🦀'.repeat(100_000)]) {
    expect(rs`let value: String = ${value}; value`).toBe(value)
    expect(rs`
      type Text = String;
      fn identity(value: Text) -> Text { value }
      identity(${value})
    `).toBe(value)
    expect(rs`let value: String = ${value}; format!("[{value}]")`).toBe(`[${value}]`)
    expect(rs`let _: String = ${value};`).toBeUndefined()
  }
})

test('replaces unpaired UTF-16 surrogates when converting to UTF-8', () => {
  for (const value of ['\uD800', '\uDC00', 'a\uD800b\uDC00']) {
    expect(rs`let value: String = ${value}; value`).toBe(value.toWellFormed())
  }
})

test('accepts multiple strings alongside other runtime types', () => {
  const left = 'hello'
  const right = '🦀'
  const other = { value: 42 }
  const number = 42

  for (const flag of [true, false]) {
    expect(rs`
      let left: String = ${left};
      let number: u32 = ${number};
      let other = ${other};
      let _: () = ${undefined};
      let flag: bool = ${flag};
      let right: String = ${right};
      let same = ${other};
      if flag && number == 42 && other.strict_eq(&same) { left + &right } else { right }
    `).toBe(flag ? 'hello🦀' : '🦀')
  }
  expect(rs`let _: String = ${left}; ${other}`).toBe(other)
  expect(rs`let left: String = ${left}; let right: String = ${right}; left == right`).toBe(false)
  expect(rs`let value: String = ${left}; value.len() as u32`).toBe(5)
})

test('string inputs reject non-strings without coercion', () => {
  const echo = (value: unknown) => rs`let value: String = ${value}; value`
  let coercions = 0
  const coercible = { toString() { coercions++; return 'text' } }

  for (const value of [undefined, null, 0, 1, NaN, true, false, 1n, Symbol('text'), {}, [], new String('text'), coercible]) {
    expect(() => echo(value)).toThrow(TypeError)
    expect(() => echo(value)).toThrow('Rustier input 0: expected string')
  }
  expect(coercions).toBe(0)
  expect(echo('text')).toBe('text')
})

test('string validation precedes Rust execution', () => {
  let calls = 0
  const callback = () => { calls++ }
  const invoke = (value: unknown) => rs`
    let callback = ${callback};
    let text: String = ${value};
    callback.apply(&[]).unwrap();
    text
  `

  expect(() => invoke(42)).toThrow('Rustier input 1: expected string')
  expect(calls).toBe(0)
  expect(invoke('text')).toBe('text')
  expect(calls).toBe(1)
})
