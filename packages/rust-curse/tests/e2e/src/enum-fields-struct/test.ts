import { expect, it } from 'vitest'
import {
  child_value,
  copy_child_to_boundary_enum_child,
  make_child_struct,
  make_first_boundary_enum,
  mutate_owned_child,
  set_child_value,
} from './test.rs'

it('returns struct-valued enum fields as live projections', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')
  const child = parent.child

  child.value = 40

  expect(parent.child.value).toBe(40)
  expect(child_value(parent.child)).toBe(40)
})

it('returns repeated enum field getters for the same Rust field', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
if (parent.type !== 'first') throw new Error('expected the first variant')
  const first = parent.child
  const second = parent.child

  expect(first).toBe(second)
  first.value = 40

  expect(second.value).toBe(40)
  expect(parent.child.value).toBe(40)
})

it('passes a projected enum field by shared reference', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')

  expect(child_value(parent.child)).toBe(30)
})

it('passes a projected enum field by mutable reference', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')

  set_child_value(parent.child, 40)

  expect(parent.child.value).toBe(40)
})

it('passes a projected enum field by value as an independent clone', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')

  const moved = mutate_owned_child(parent.child, 99)

  expect(moved.value).toBe(99)
  expect(parent.child.value).toBe(30)
})

it('sets a struct-valued enum field by cloning the assigned wrapper', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  const replacement = make_child_struct(40)
  if (parent.type !== 'first') throw new Error('expected the first variant')

  parent.child = replacement
  replacement.value = 50

  expect(parent.child.value).toBe(40)
  expect(replacement.value).toBe(50)
})

it('detaches an existing projected wrapper when an enum field is replaced', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')
  const oldChild = parent.child

  parent.child = make_child_struct(40)
  oldChild.value = 99

  expect(oldChild.value).toBe(99)
  expect(parent.child.value).toBe(40)
})

it('detaches all cached wrappers when an enum field is replaced', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  if (parent.type !== 'first') throw new Error('expected the first variant')
  const firstOldChild = parent.child
  const secondOldChild = parent.child

  parent.child = make_child_struct(40)
  firstOldChild.value = 99

  expect(secondOldChild.value).toBe(99)
  expect(parent.child.value).toBe(40)
})

it('passes a projected enum field and its parent as distinct mutable roots', () => {
  const parent = make_first_boundary_enum(10, 20, 30)
  const source = make_child_struct(40)
  if (parent.type !== 'first') throw new Error('expected the first variant')

  copy_child_to_boundary_enum_child(source, parent)

  expect(parent.child.value).toBe(40)
  expect(source.value).toBe(40)
})
