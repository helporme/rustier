import { expect, it } from 'vitest'
import {
  boundary_enum_shared,
  make_boundary_enum_container,
  make_boundary_enum_holder,
  make_unit_shape,
} from './test.rs'

it('throws when a different enum type is passed to a function', () => {
  const other = make_unit_shape()

  expect(() => boundary_enum_shared(other as never)).toThrow()
})

it('throws when a different enum type is assigned to a struct field', () => {
  const holder = make_boundary_enum_holder(10, 20, 30)
  const other = make_unit_shape()

  expect(() => {
    holder.value = other as never
  }).toThrow()
})

it('throws when a different enum type is assigned to an enum variant field', () => {
  const container = make_boundary_enum_container(10, 20, 30)
  const other = make_unit_shape()
  if (container.type !== 'occupied') throw new Error('expected the occupied variant')

  expect(() => {
    container.value = other as never
  }).toThrow()
})
