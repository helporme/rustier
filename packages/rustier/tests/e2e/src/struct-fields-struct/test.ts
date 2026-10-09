import { expect, it } from 'vitest'
import {
  child_value,
  copy_child_to_parent_child,
  make_child_struct,
  make_grandparent_struct,
  make_non_clone_parent_struct,
  make_parent_struct,
  mutate_owned_child,
  non_clone_child_value,
  parent_child_value,
  set_child_value,
  set_non_clone_child_value,
} from './test.rs'

it('returns nested struct fields as live projections', () => {
  const parent = make_parent_struct(10)
  const child = parent.child

  child.value = 20

  expect(parent.child.value).toBe(20)
  expect(parent_child_value(parent)).toBe(20)
})

it('returns repeated nested getters that point to the same Rust field', () => {
  const parent = make_parent_struct(10)
  const first = parent.child
  const second = parent.child

  first.value = 20

  expect(second.value).toBe(20)
  expect(parent.child.value).toBe(20)
})

it('passes a projected child by shared reference', () => {
  const parent = make_parent_struct(10)

  expect(child_value(parent.child)).toBe(10)

  parent.child.value = 20

  expect(child_value(parent.child)).toBe(20)
})

it('passes a projected child by mutable reference', () => {
  const parent = make_parent_struct(10)

  expect(parent.child.value).toBe(10)
  set_child_value(parent.child, 20)

  expect(parent.child.value).toBe(20)
  expect(parent_child_value(parent)).toBe(20)
})

it('passes a projected child by value as an independent clone', () => {
  const parent = make_parent_struct(10)
  const moved = mutate_owned_child(parent.child, 99)

  expect(moved.value).toBe(99)
  expect(parent.child.value).toBe(10)
})

it('composes projections through deeply nested struct fields', () => {
  const grandparent = make_grandparent_struct(10)

  grandparent.parent.child.value = 20

  expect(grandparent.parent.child.value).toBe(20)
})

it('sets a struct field by cloning the assigned wrapper', () => {
  const parent = make_parent_struct(1)
  const replacement = make_child_struct(10)

  parent.child = replacement
  replacement.value = 20

  expect(parent.child.value).toBe(10)
  expect(replacement.value).toBe(20)
})

it('detaches an existing projected child wrapper when the parent field is replaced', () => {
  const parent = make_parent_struct(1)
  const oldChild = parent.child

  parent.child = make_child_struct(10)
  oldChild.value = 99

  expect(oldChild.value).toBe(99)
  expect(parent.child.value).toBe(10)
})

it('detaches all cached wrappers for an old child when the parent field is replaced', () => {
  const parent = make_parent_struct(1)
  const firstOldChild = parent.child
  const secondOldChild = parent.child

  parent.child = make_child_struct(10)
  firstOldChild.value = 99

  expect(secondOldChild.value).toBe(99)
  expect(parent.child.value).toBe(10)
})

it('uses a non-clone nested struct through projection', () => {
  const parent = make_non_clone_parent_struct(10)
  const child = parent.child

  expect(non_clone_child_value(child)).toBe(10)

  set_non_clone_child_value(child, 20)

  expect(non_clone_child_value(parent.child)).toBe(20)
})

it('passes projected children and parent wrappers as distinct mutable roots', () => {
  const parent = make_parent_struct(1)
  const source = make_child_struct(20)

  copy_child_to_parent_child(source, parent)

  expect(parent.child.value).toBe(20)
  expect(source.value).toBe(20)
})
