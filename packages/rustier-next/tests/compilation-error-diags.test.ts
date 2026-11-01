import { expect, test } from 'vitest'
import { rs } from 'rustier-next'
import { rustierDiags } from '$build/diags'

const diagnostics = rustierDiags.filter(message => message.includes('/compilation-error-diags.test.ts:'))

test('invalid let statements report diagnostics and throw', () => {
  const invalid = () => rs`let = ;`
  const error = diagnostics.find(message => message.includes('rs`let = ;`'))

  expect(error).toMatch(/^syntax-error/)
  expect(invalid).toThrow(error)
})

test('incomplete function declarations report diagnostics and throw', () => {
  const invalid = () => rs`fn broken( {`
  const error = diagnostics.find(message => message.includes('rs`fn broken( {`'))

  expect(error).toMatch(/^syntax-error/)
  expect(invalid).toThrow(error)
})

test('invalid expressions report diagnostics with source code and throw', () => {
  const invalid = () => rs`{x} + 3`
  const error = diagnostics.find(message => message.includes('rs`{x} + 3`'))

  expect(error).toMatch(/^syntax-error/)
  expect(error).toContain('expected expression, item or let statement')
  expect(invalid).toThrow(error)
})
