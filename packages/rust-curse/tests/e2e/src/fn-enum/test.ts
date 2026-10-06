import { expect, it } from 'vitest'
import {
  circle,
  resize_owned_circle,
  same_shape_refs,
  set_circle_radius,
  shape_layer,
} from './test.rs'

it('constructs Shape::Circle with a radius, layer, and center', () => {
  const shape = circle(10, 20, 30)

  expect(shape.type).toBe('Circle')
  if (shape.type !== 'Circle') throw new Error('expected Shape::Circle')

  expect(shape.radius).toBe(10)
  expect(shape.layer).toBe(20)
  expect(shape.center.x).toBe(30)
})

it('reads Shape::Circle.layer and compares identity through shared Shape references', () => {
  const shape = circle(10, 20, 30)

  expect(shape_layer(shape)).toBe(20)
  expect(same_shape_refs(shape, shape)).toBe(true)
})

it('sets Shape::Circle.radius through a mutable Shape reference', () => {
  const shape = circle(10, 20, 30)

  set_circle_radius(shape, 99)

  expect(shape.type).toBe('Circle')
  if (shape.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(shape.radius).toBe(99)
})

it('resizes an owned Shape::Circle clone without changing the original JS wrapper', () => {
  const original = circle(10, 20, 30)
  const resized = resize_owned_circle(original, 40, 50)

  if (original.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(original.radius).toBe(10)
  expect(original.center.x).toBe(30)

  if (resized.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(resized.radius).toBe(40)
  expect(resized.center.x).toBe(50)
})
