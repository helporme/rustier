import { expect, it } from 'vitest'
import {
  boundary_enum_and_child_value,
  copy_boundary_enum,
  make_first_boundary_enum,
  read_and_mutate_child,
  set_two_boundary_enums,
} from './test.rs'

it('throws when the same enum is passed as two mutable references', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  expect(() => set_two_boundary_enums(value, value)).toThrow()
})

it('throws when the same enum is passed as shared and mutable references', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  expect(() => copy_boundary_enum(value, value)).toThrow()
})

it('throws when an enum reference aliases a mutable projected child', () => {
  const value = make_first_boundary_enum(10, 20, 30)
  if (value.type !== 'first') throw new Error('expected the first variant')

  expect(() => boundary_enum_and_child_value(value, value.child)).toThrow()
})

it('throws when repeated enum field getters alias shared and mutable references', () => {
  const value = make_first_boundary_enum(10, 20, 30)
  if (value.type !== 'first') throw new Error('expected the first variant')
  const read = value.child
  const mutate = value.child

  expect(() => read_and_mutate_child(read, mutate)).toThrow()
})
