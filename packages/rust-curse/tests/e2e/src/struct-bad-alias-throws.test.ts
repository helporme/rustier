import { expect, it } from 'vitest'
import {
  copy_counter_value,
  make_counter_struct,
  make_parent_struct,
  parent_and_child_value,
  read_and_mutate_child,
  set_two_counter_values,
} from './lib.rs'

// todo: add good runtime error message here

it('throws when the same struct is passed as two mutable references', () => {
  const value = make_counter_struct(10)

  expect(() => set_two_counter_values(value, value)).toThrow()
})

it('throws when the same struct is passed as shared and mutable references', () => {
  const value = make_counter_struct(10)

  expect(() => copy_counter_value(value, value)).toThrow()
})

it('throws when a parent reference aliases a mutable projected child', () => {
  const parent = make_parent_struct(10)

  expect(() => parent_and_child_value(parent, parent.child)).toThrow()
})

it('throws when repeated child getters alias shared and mutable references', () => {
  const parent = make_parent_struct(10)
  const read = parent.child
  const mutate = parent.child

  expect(() => read_and_mutate_child(read, mutate)).toThrow()
})
