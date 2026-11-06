import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('moving invalidates every alias for reads, mutations, and subsequent moves', () => {
  const message = rs`handy_crate::Message("Hello")`
  const alias = message
  const read = () => rs`
    let message: &handy_crate::Message = ${alias};
    message.0.to_owned()
  `
  const update = () => rs`
    let message: &mut handy_crate::Message = ${message};
    message.0 = "New text";
  `
  const move = () => rs`
    let message: handy_crate::Message = ${message};
    message.0.to_owned()
  `

  expect(read()).toBe('Hello')
  expect(move()).toBe('Hello')

  for (let attempt = 0; attempt < 2; attempt++) {
    expect(read).toThrow(/moved/i)
    expect(update).toThrow(/moved/i)
    expect(move).toThrow(/moved/i)
  }
})

test('returning a moved value creates a live handle without reviving the old one', () => {
  const original = rs`handy_crate::Message("Hello")`
  const transferred = rs`
    let message: handy_crate::Message = ${original};
    message
  `
  const read = (value: unknown) => rs`
    let message: &handy_crate::Message = ${value};
    message.0.to_owned()
  `

  expect(read(transferred)).toBe('Hello')
  expect(() => read(original)).toThrow(/moved/i)
  expect(rs`
    let message: handy_crate::Message = ${transferred};
    message.0.to_owned()
  `).toBe('Hello')
  expect(() => read(transferred)).toThrow(/moved/i)
  expect(() => read(original)).toThrow(/moved/i)
})

test('by-value inputs invalidate handles even when the Rust type implements Copy', () => {
  const value = rs`handy_crate::CopyValue(42)`
  const take = () => rs`
    let value: handy_crate::CopyValue = ${value};
    value.0
  `

  expect(take()).toBe(42)
  expect(take).toThrow(/moved/i)
})

test('moving one allocation does not invalidate another of the same type', () => {
  const create = () => rs`handy_crate::Message("Hello")`
  const first = create()
  const second = create()

  rs`let value: handy_crate::Message = ${first}; drop(value);`
  expect(rs`
    let value: &handy_crate::Message = ${second};
    value.0.to_owned()
  `).toBe('Hello')
  expect(() => rs`
    let value: &handy_crate::Message = ${first};
    value.0.to_owned()
  `).toThrow(/moved/i)
})
