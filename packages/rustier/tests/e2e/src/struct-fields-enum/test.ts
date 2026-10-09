import { expect, it } from 'vitest'
import {
  canvas_with_circle,
  circle,
  replace_with_square,
  set_circle_radius,
  shape_layer,
} from './test.rs'

it('projects Canvas.shape as a live Shape wrapper', () => {
  const canvas = canvas_with_circle(10, 20, 30)
  const projectedShape = canvas.shape

  replace_with_square(projectedShape, 40, 50)

  expect(canvas.shape.type).toBe('Square')
  if (canvas.shape.type !== 'Square') throw new Error('expected Shape::Square')
  expect(canvas.shape.layer).toBe(40)
  expect(canvas.shape.center.position).toBe(50)
})

it('reuses the wrapper returned by repeated Canvas.shape getters', () => {
  const canvas = canvas_with_circle(10, 20, 30)

  expect(canvas.shape).toBe(canvas.shape)
  expect(shape_layer(canvas.shape)).toBe(20)
})

it('composes projections through Canvas.shape and Shape::Circle.center', () => {
  const canvas = canvas_with_circle(10, 20, 30)
  const projectedShape = canvas.shape
  if (projectedShape.type !== 'Circle') throw new Error('expected Shape::Circle')

  projectedShape.center.position = 40

  const currentShape = canvas.shape
  if (currentShape.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(currentShape.center.position).toBe(40)
})

it('sets Canvas.shape by cloning the assigned Shape wrapper', () => {
  const canvas = canvas_with_circle(1, 2, 3)
  const replacement = circle(10, 20, 30)

  canvas.shape = replacement
  set_circle_radius(replacement, 99)

  if (canvas.shape.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(canvas.shape.radius).toBe(10)
  if (replacement.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(replacement.radius).toBe(99)
})

it('detaches an existing Shape projection when Canvas.shape is replaced', () => {
  const canvas = canvas_with_circle(1, 2, 3)
  const oldShape = canvas.shape

  canvas.shape = circle(10, 20, 30)
  set_circle_radius(oldShape, 99)

  if (oldShape.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(oldShape.radius).toBe(99)
  if (canvas.shape.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(canvas.shape.radius).toBe(10)
})
