import { expect, it } from 'vitest'
import { make_child_struct, make_parent_struct } from './test.rs'

it('throws when a primitive field setter receives the wrong JS type', () => {
  const child = make_child_struct(10)

  expect(() => {
    child.value = Symbol('wrong type') as never
  }).toThrow()
})

it('throws when a number is assigned to a struct-valued field', () => {
  const parent = make_parent_struct(10)

  expect(() => {
    parent.child = 42 as never
  }).toThrow()
})
