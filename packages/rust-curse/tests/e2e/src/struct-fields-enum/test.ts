import { expect, it } from 'vitest'
import {
  boundary_enum_shared,
  make_boundary_enum_holder,
  make_first_boundary_enum,
  replace_boundary_enum_with_second,
  set_boundary_enum_unique,
} from './test.rs'

it('returns an enum-valued struct field as a live projection', () => {
  const holder = make_boundary_enum_holder(10, 20, 30)
  const projected = holder.value

  replace_boundary_enum_with_second(projected, 40, 50)

  expect(holder.value.type).toBe('second')
  if (holder.value.type !== 'second') throw new Error('expected the second variant')
  expect(holder.value.shared).toBe(40)
  expect(holder.value.child.value).toBe(50)
})

it('returns repeated enum-valued struct field getters for the same Rust field', () => {
  const holder = make_boundary_enum_holder(10, 20, 30)

  expect(holder.value).toBe(holder.value)
  expect(boundary_enum_shared(holder.value)).toBe(20)
})

it('composes projections through enum-valued struct fields', () => {
  const holder = make_boundary_enum_holder(10, 20, 30)
  const projected = holder.value
  if (projected.type !== 'first') throw new Error('expected the first variant')

  projected.child.value = 40

  const current = holder.value
  if (current.type !== 'first') throw new Error('expected the first variant')
  expect(current.child.value).toBe(40)
})

it('sets an enum-valued struct field by cloning the assigned wrapper', () => {
  const holder = make_boundary_enum_holder(1, 2, 3)
  const replacement = make_first_boundary_enum(10, 20, 30)

  holder.value = replacement
  set_boundary_enum_unique(replacement, 99)

  if (holder.value.type !== 'first') throw new Error('expected the first variant')
  expect(holder.value.unique).toBe(10)
  if (replacement.type !== 'first') throw new Error('expected the first variant')
  expect(replacement.unique).toBe(99)
})

it('detaches an existing enum projection when the struct field is replaced', () => {
  const holder = make_boundary_enum_holder(1, 2, 3)
  const oldValue = holder.value

  holder.value = make_first_boundary_enum(10, 20, 30)
  set_boundary_enum_unique(oldValue, 99)

  if (oldValue.type !== 'first') throw new Error('expected the first variant')
  expect(oldValue.unique).toBe(99)
  if (holder.value.type !== 'first') throw new Error('expected the first variant')
  expect(holder.value.unique).toBe(10)
})
