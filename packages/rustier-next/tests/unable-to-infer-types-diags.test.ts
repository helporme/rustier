import { expect, test } from 'vitest'
import { rs } from 'rustier-next'
import { rustierDiags } from '$build/diags'

const diagnostics = rustierDiags.filter(message => message.includes('/unable-to-infer-types-diags.test.ts:'))

test('unconstrained types report diagnostics and throw', () => {
  const invalid = () => rs`let values = Vec::new();`
  const error = diagnostics.find(message => message.startsWith('E0282'))

  expect(error).toContain('type annotations needed')
  expect(error).toContain('rs`let values = Vec::new();`')
  expect(invalid).toThrow(error)
})

test('unresolved interpolation types report the failed trait bound and throw', () => {
  const x = 10
  const invalid = () => rs`${x} + 3`
  const error = diagnostics.find(message => message.includes('JsValue: Add<i32>'))

  expect(error).toMatch(/^E0277/)
  expect(error).toContain('rs`${x} + 3`')
  expect(invalid).toThrow(error)
})

test('incompatible operand types report the failed trait bound and throw', () => {
  const invalid = () => rs`2 + ""`
  const error = diagnostics.find(message => message.includes('i32: Add<&str>'))

  expect(error).toMatch(/^E0277/)
  expect(error).toContain('rs`2 + ""`')
  expect(invalid).toThrow(error)
})
