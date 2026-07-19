import { expect, it } from 'vitest'
import {
  make_circle,
  make_hidden,
  make_line,
  make_oval,
  make_point,
} from './test.rs'

it('returns Shape::Circle with writable fields and a circle type tag', () => {
  const shape = make_circle(2.5, 10, 20)

  expect(shape.type).toBe('circle')
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  expect(shape.radius).toBe(2.5)
  expect(shape.center[0]).toBe(10)
  expect(shape.center[1]).toBe(20)

  shape.radius = 3.5
  shape.center[0] = 30

  expect(shape.radius).toBe(3.5)
  expect(shape.center[0]).toBe(30)
})

it('returns Shape::Oval with named fields and an oval type tag', () => {
  const shape = make_oval(2.5, 3.5, 10, 20)

  expect(shape.type).toBe('oval')
  if (shape.type !== 'oval') throw new Error('expected Shape::Oval')

  expect(shape.radius_x).toBe(2.5)
  expect(shape.radius_y).toBe(3.5)
  expect(shape.center[0]).toBe(10)
  expect(shape.center[1]).toBe(20)
})

it('returns Shape::Point as a single-field tuple variant with a point type tag', () => {
  const shape = make_point(10, 20)

  expect(shape.type).toBe('point')
  if (shape.type !== 'point') throw new Error('expected Shape::Point')

  expect(shape[0][0]).toBe(10)
  expect(shape[0][1]).toBe(20)
})

it('returns Shape::Line as a two-field tuple variant with a line type tag', () => {
  const shape = make_line(10, 20, 30, 40)

  expect(shape.type).toBe('line')
  if (shape.type !== 'line') throw new Error('expected Shape::Line')

  expect(shape[0][0]).toBe(10)
  expect(shape[0][1]).toBe(20)
  expect(shape[1][0]).toBe(30)
  expect(shape[1][1]).toBe(40)
})

it('returns Shape::Hidden as a unit variant with a hidden type tag', () => {
  const shape = make_hidden()

  expect(shape.type).toBe('hidden')
})
