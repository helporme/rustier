import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('handles are opaque and cannot be forged from JS', () => {
  const message = rs`handy_crate::Message("Hello")`
  const read = (value: unknown) => rs`
    let message: &handy_crate::Message = ${value};
    message.0.to_owned()
  `

  expect(message.constructor.name).toBe('Rs')
  expect(Reflect.ownKeys(message)).toEqual([])
  expect(() => new message.constructor(0n)).toThrow(TypeError)

  for (const invalid of [0, 0n, null, undefined, {}, Object.create(Object.getPrototypeOf(message)), new Proxy(message, {})]) {
    expect(() => read(invalid)).toThrow(TypeError)
  }
  expect(read(message)).toBe('Hello')
})

test('wrong Rust types fail without consuming the handle', () => {
  const message = rs`handy_crate::Message("Hello")`

  expect(() => rs`let value: Vec<u32> = ${message}; drop(value);`).toThrow(/incompatible type/i)
  expect(() => rs`let value: &Vec<u32> = ${message}; value.len() as u32`).toThrow(/incompatible type/i)
  expect(() => rs`let value: &mut Vec<u32> = ${message}; value.clear();`).toThrow(/incompatible type/i)
  expect(rs`let value: handy_crate::Message = ${message}; value.0.to_owned()`).toBe('Hello')
})

test('different generic arguments fail without consuming or mutating the handle', () => {
  const values = rs`vec![1, 2, 3]`
  rs`Vec::<i32>::push(${values}, 4)`

  expect(() => rs`let values: &Vec<u32> = ${values}; values[3]`).toThrow(/incompatible type/i)
  expect(() => rs`let values: &mut Vec<u32> = ${values}; values.clear();`).toThrow(/incompatible type/i)
  expect(() => rs`let values: Vec<u32> = ${values}; drop(values);`).toThrow(/incompatible type/i)
  expect(rs`let values: Vec<i32> = ${values}; values.into_iter().sum::<i32>()`).toBe(10)
})
