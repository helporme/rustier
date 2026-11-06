import { expect, test } from 'vitest'
import { rs } from 'rustier-next'
import { rustierDiags } from '$build/diags'

const diagnostics = rustierDiags.filter(message => message.includes('/handle-lifetimes-throws.test.ts:'))

test('ADTs with static lifetime arguments report diagnostics and throw', () => {
  const invalid = () => rs`handy_crate::Borrowed("Hello")`
  const error = diagnostics.find(message => message.includes('rs`handy_crate::Borrowed("Hello")`'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('lifetimes hidden behind aliases report diagnostics and throw', () => {
  const invalid = () => rs`type StaticBorrow = handy_crate::Borrowed<'static>; StaticBorrow { 0: "Hello" }`
  const error = diagnostics.find(message => message.includes('type StaticBorrow'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('nested lifetime arguments report diagnostics and throw', () => {
  const invalid = () => rs`Some(handy_crate::Borrowed("Hello"))`
  const error = diagnostics.find(message => message.includes('rs`Some(handy_crate::Borrowed'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})
