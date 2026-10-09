import { expect, it } from 'vitest'
import {
  copy_shape,
  hide_two_shapes,
  make_circle,
  read_and_mutate_center,
  read_shape_and_mutate_center,
} from './test.rs'

it('throws when the same Circle is passed as two mutable Shape references', () => {
  const circle = make_circle(10, 20, 30)

  expect(() => hide_two_shapes(circle, circle)).toThrow()
})

it('throws when the same Circle is passed as shared and mutable Shape references', () => {
  const circle = make_circle(10, 20, 30)

  expect(() => copy_shape(circle, circle)).toThrow()
})

it('throws when Shape::Circle aliases its mutable center projection', () => {
  const circle = make_circle(10, 20, 30)
  if (circle.type !== 'Circle') throw new Error('expected the Circle variant')

  expect(() => read_shape_and_mutate_center(circle, circle.center)).toThrow()
})

it('throws when repeated Circle.center getters alias shared and mutable references', () => {
  const circle = make_circle(10, 20, 30)
  if (circle.type !== 'Circle') throw new Error('expected the Circle variant')
  const read = circle.center
  const mutate = circle.center

  expect(() => read_and_mutate_center(read, mutate)).toThrow()
})
