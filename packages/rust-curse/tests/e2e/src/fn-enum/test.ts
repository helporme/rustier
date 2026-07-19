import { expect, it } from 'vitest'
import {
  boundary_enum_shared,
  make_first_boundary_enum,
  mutate_owned_boundary_enum,
  same_boundary_enum_refs,
  set_boundary_enum_unique,
} from './test.rs'

it('returns an enum from a function', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  expect(value.type).toBe('first')
  if (value.type !== 'first') throw new Error('expected the first variant')

  expect(value.unique).toBe(10)
  expect(value.shared).toBe(20)
  expect(value.child.value).toBe(30)
})

it('passes an enum by shared reference without cloning it', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  expect(boundary_enum_shared(value)).toBe(20)
  expect(same_boundary_enum_refs(value, value)).toBe(true)
})

it('passes an enum by mutable reference and reflects field mutations in JS', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  set_boundary_enum_unique(value, 99)

  expect(value.type).toBe('first')
  if (value.type !== 'first') throw new Error('expected the first variant')
  expect(value.unique).toBe(99)
})

it('moves an enum by cloning and leaves the original JS wrapper unchanged', () => {
  const original = make_first_boundary_enum(10, 20, 30)
  const moved = mutate_owned_boundary_enum(original, 40, 50)

  if (original.type !== 'first') throw new Error('expected the first variant')
  expect(original.unique).toBe(10)
  expect(original.child.value).toBe(30)

  if (moved.type !== 'first') throw new Error('expected the first variant')
  expect(moved.unique).toBe(40)
  expect(moved.child.value).toBe(50)
})
