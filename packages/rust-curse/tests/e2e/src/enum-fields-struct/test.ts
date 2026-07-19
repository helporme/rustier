import { expect, it } from 'vitest'
import {
  circle,
  copy_point_to_shape_center,
  move_owned_point,
  point,
  point_position,
  set_point_position,
} from './test.rs'

it('projects Shape::Circle.center as a live Point wrapper', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')
  const center = shape.center

  center.position = 40

  expect(shape.center.position).toBe(40)
  expect(point_position(shape.center)).toBe(40)
})

it('reuses the wrapper returned by repeated Shape::Circle.center getters', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')
  const firstCenter = shape.center
  const secondCenter = shape.center

  expect(firstCenter).toBe(secondCenter)
  firstCenter.position = 40

  expect(secondCenter.position).toBe(40)
  expect(shape.center.position).toBe(40)
})

it('passes Shape::Circle.center by shared reference', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  expect(point_position(shape.center)).toBe(30)
})

it('passes Shape::Circle.center by mutable reference', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  set_point_position(shape.center, 40)

  expect(shape.center.position).toBe(40)
})

it('passes Shape::Circle.center by value as an independent owned clone', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  const moved = move_owned_point(shape.center, 99)

  expect(moved.position).toBe(99)
  expect(shape.center.position).toBe(30)
})

it('sets Shape::Circle.center by cloning the assigned Point wrapper', () => {
  const shape = circle(10, 20, 30)
  const replacement = point(40)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  shape.center = replacement
  replacement.position = 50

  expect(shape.center.position).toBe(40)
  expect(replacement.position).toBe(50)
})

it('detaches an existing Point projection when Shape::Circle.center is replaced', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')
  const oldCenter = shape.center

  shape.center = point(40)
  oldCenter.position = 99

  expect(oldCenter.position).toBe(99)
  expect(shape.center.position).toBe(40)
})

it('detaches all cached Point wrappers when Shape::Circle.center is replaced', () => {
  const shape = circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')
  const firstOldCenter = shape.center
  const secondOldCenter = shape.center

  shape.center = point(40)
  firstOldCenter.position = 99

  expect(secondOldCenter.position).toBe(99)
  expect(shape.center.position).toBe(40)
})

it('passes Point and Shape::Circle.center as distinct alias roots', () => {
  const shape = circle(10, 20, 30)
  const source = point(40)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')

  copy_point_to_shape_center(source, shape)

  expect(shape.center.position).toBe(40)
  expect(source.position).toBe(40)
})
