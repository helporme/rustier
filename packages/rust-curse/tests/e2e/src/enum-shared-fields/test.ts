import { expect, it } from 'vitest'
import {
  make_first_boundary_enum,
  make_number_shared_field_enum,
  make_second_boundary_enum,
  make_text_shared_field_enum,
} from './test.rs'

it('reads and writes fields with the same name and type on different variants', () => {
  const first = make_first_boundary_enum(10, 20, 30)
  const second = make_second_boundary_enum(40, 50)
  if (first.type !== 'first') throw new Error('expected the first variant')
  if (second.type !== 'second') throw new Error('expected the second variant')

  expect(first.shared).toBe(20)
  expect(second.shared).toBe(40)

  first.shared = 60
  second.shared = 70

  expect(first.shared).toBe(60)
  expect(second.shared).toBe(70)
})

it('reads and writes fields with the same name and different types on different variants', () => {
  const number = make_number_shared_field_enum(10)
  const text = make_text_shared_field_enum('initial 🦀')
  if (number.type !== 'number') throw new Error('expected the number variant')
  if (text.type !== 'text') throw new Error('expected the text variant')

  expect(number.value).toBe(10)
  expect(text.value).toBe('initial 🦀')

  number.value = 20
  text.value = 'updated 🦀'

  expect(number.value).toBe(20)
  expect(text.value).toBe('updated 🦀')
})
