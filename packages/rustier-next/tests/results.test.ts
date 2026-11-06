import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('returns results with a concrete String error type', () => {
  const get = (fail: boolean) => rs`
    type Output = Result<u32, String>;
    fn get(fail: bool) -> Output {
      if fail { Err("héllo\0🦀".to_owned()) } else { Ok(20) }
    }
    get(${fail})
  `

  for (let i = 0; i < 3; i++) {
    expect(get(false)).toBe(20)
    expect(() => get(true)).toThrowError(expect.objectContaining({
      name: 'RustError',
      message: 'héllo\0🦀',
    }))
    expect(get(false)).toBe(20)
  }
})

test('returns results with a concrete std::error::Error implementation', () => {
  const get = (fail: boolean) => rs`
    fn get(fail: bool) -> Result<String, std::io::Error> {
      if fail { Err(std::io::Error::other("io error")) } else { Ok("hello".to_owned()) }
    }
    get(${fail})
  `

  expect(get(false)).toBe('hello')
  expect(() => get(true)).toThrowError(expect.objectContaining({
    name: 'RustError',
    message: 'io error',
  }))
  expect(get(false)).toBe('hello')
})

test('defaults the success type of error-only results to unit', () => {
  const failures = [
    () => rs`Err("Error".to_owned())`,
    () => rs`return Err("Error".to_owned());`,
    () => rs`Err(std::io::Error::other("Error"))`,
  ]

  for (const fail of failures) {
    expect(fail).toThrowError(expect.objectContaining({
      name: 'RustError',
      message: 'Error',
    }))
  }
})

test('infers a boxed error type for otherwise unconstrained Ok expressions', () => {
  expect(rs`Ok(20u32)`).toBe(20)
  expect(rs`Ok("hello".to_owned())`).toBe('hello')
  expect(rs`Ok(true)`).toBe(true)
  expect(rs`Ok(())`).toBeUndefined()
  expect(rs`return Ok(7u32);`).toBe(7)

  const value = { answer: 42 }
  expect(rs`Ok(${value})`).toBe(value)
})

test('propagates different error types into an unconstrained result with question marks', () => {
  const get = (failString: boolean, failIo: boolean) => rs`
    fn first(fail: bool) -> Result<u32, String> {
      if fail { Err("string error".to_owned()) } else { Ok(10) }
    }
    fn second(fail: bool) -> Result<u32, std::io::Error> {
      if fail { Err(std::io::Error::other("io error")) } else { Ok(2) }
    }
    let x = first(${failString})?;
    let y = second(${failIo})?;
    Ok(x * y)
  `

  expect(get(false, false)).toBe(20)
  expect(() => get(true, false)).toThrowError(expect.objectContaining({
    name: 'RustError',
    message: 'string error',
  }))
  expect(() => get(false, true)).toThrowError(expect.objectContaining({
    name: 'RustError',
    message: 'io error',
  }))
  expect(get(false, false)).toBe(20)
})
