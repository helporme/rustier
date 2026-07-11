import { expect, it } from 'vitest'
import { make_string_field_struct, string_field_value } from './lib.rs'

it('reads and writes string struct fields', () => {
  const value = make_string_field_struct('initial 🦀')

  expect(value.value).toBe('initial 🦀')

  value.value = 'updated 🦀'

  expect(value.value).toBe('updated 🦀')
  expect(string_field_value(value)).toBe('updated 🦀')
})
