import { expect, it } from 'vitest'
import {
  make_boundary_enum_container,
  make_first_boundary_enum,
  replace_boundary_enum_with_second,
  set_boundary_enum_unique,
} from './test.rs'

it('returns an enum-valued enum variant field as a live projection', () => {
  const container = make_boundary_enum_container(10, 20, 30)
  if (container.type !== 'occupied') throw new Error('expected the occupied variant')
  const projected = container.value

  replace_boundary_enum_with_second(projected, 40, 50)

  expect(container.value.type).toBe('second')
  if (container.value.type !== 'second') throw new Error('expected the second variant')
  expect(container.value.shared).toBe(40)
  expect(container.value.child.value).toBe(50)
})

it('returns repeated enum-valued enum field getters for the same Rust field', () => {
  const container = make_boundary_enum_container(10, 20, 30)
  if (container.type !== 'occupied') throw new Error('expected the occupied variant')

  expect(container.value).toBe(container.value)
})

it('sets an enum-valued enum field by cloning the assigned wrapper', () => {
  const container = make_boundary_enum_container(1, 2, 3)
  const replacement = make_first_boundary_enum(10, 20, 30)
  if (container.type !== 'occupied') throw new Error('expected the occupied variant')

  container.value = replacement
  set_boundary_enum_unique(replacement, 99)

  if (container.value.type !== 'first') throw new Error('expected the first variant')
  expect(container.value.unique).toBe(10)
  if (replacement.type !== 'first') throw new Error('expected the first variant')
  expect(replacement.unique).toBe(99)
})

it('detaches an existing enum projection when the enum field is replaced', () => {
  const container = make_boundary_enum_container(1, 2, 3)
  if (container.type !== 'occupied') throw new Error('expected the occupied variant')
  const oldValue = container.value

  container.value = make_first_boundary_enum(10, 20, 30)
  set_boundary_enum_unique(oldValue, 99)

  if (oldValue.type !== 'first') throw new Error('expected the first variant')
  expect(oldValue.unique).toBe(99)
  if (container.value.type !== 'first') throw new Error('expected the first variant')
  expect(container.value.unique).toBe(10)
})
