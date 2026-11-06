import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('shared borrows preserve a non-Clone value across calls', () => {
  const message = rs`handy_crate::Message("Hello")`
  const read = () => rs`
    let message: &handy_crate::Message = ${message};
    message.0.to_owned()
  `

  expect(read()).toBe('Hello')
  expect(read()).toBe('Hello')
})

test('mutable borrows update the same allocation through JS aliases', () => {
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

  expect(read()).toBe('Hello')
  expect(update()).toBeUndefined()
  expect(read()).toBe('New text')
  expect(update()).toBeUndefined()
  expect(read()).toBe('New text')
})
