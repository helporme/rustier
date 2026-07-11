import { expect, it } from 'vitest'
import { child_value, make_child_struct, make_tuple_struct } from './lib.rs'

it('reads and writes primitive struct fields', () => {
  const child = make_child_struct(10)

  expect(child.value).toBe(10)

  child.value = 20

  expect(child.value).toBe(20)
  expect(child_value(child)).toBe(20)
})

it('reads and writes sparse primitive tuple struct fields', () => {
  const tuple = make_tuple_struct(10)

  expect(0 in tuple).toBe(false)
  expect(tuple[1]).toBe(10)

  tuple[1] = 20

  expect(0 in tuple).toBe(false)
  expect(tuple[1]).toBe(20)
})
