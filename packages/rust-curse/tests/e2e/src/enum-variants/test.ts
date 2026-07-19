import { expect, it } from 'vitest'
import {
  make_circle_shape,
  make_line_shape,
  make_oval_shape,
  make_point_shape,
  make_unit_shape,
} from './test.rs'

it('returns a struct variant with accessible fields and a type tag', () => {
  const shape = make_circle_shape(2.5, 10, 20)

  expect(shape.type).toBe('circle')
  if (shape.type !== 'circle') throw new Error('expected the circle variant')

  expect(shape.radius).toBe(2.5)
  expect(shape.center[0]).toBe(10)
  expect(shape.center[1]).toBe(20)

  shape.radius = 3.5
  shape.center[0] = 30

  expect(shape.radius).toBe(3.5)
  expect(shape.center[0]).toBe(30)
})

it('returns another struct variant with the expected type tag', () => {
  const shape = make_oval_shape(2.5, 3.5, 10, 20)

  expect(shape.type).toBe('oval')
  if (shape.type !== 'oval') throw new Error('expected the oval variant')

  expect(shape.radius_x).toBe(2.5)
  expect(shape.radius_y).toBe(3.5)
  expect(shape.center[0]).toBe(10)
  expect(shape.center[1]).toBe(20)
})

it('returns a single-field tuple variant with accessible numeric fields', () => {
  const shape = make_point_shape(10, 20)

  expect(shape.type).toBe('point')
  if (shape.type !== 'point') throw new Error('expected the point variant')

  expect(shape[0][0]).toBe(10)
  expect(shape[0][1]).toBe(20)
})

it('returns a multi-field tuple variant with accessible numeric fields', () => {
  const shape = make_line_shape(10, 20, 30, 40)

  expect(shape.type).toBe('line')
  if (shape.type !== 'line') throw new Error('expected the line variant')

  expect(shape[0][0]).toBe(10)
  expect(shape[0][1]).toBe(20)
  expect(shape[1][0]).toBe(30)
  expect(shape[1][1]).toBe(40)
})

it('returns a unit variant in a data-carrying enum', () => {
  const shape = make_unit_shape()

  expect(shape.type).toBe('unit')
})
