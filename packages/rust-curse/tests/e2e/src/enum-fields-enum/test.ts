import { expect, it } from 'vitest'
import {
  make_circle,
  make_selected_drawing,
  replace_with_square,
  set_circle_radius,
} from './test.rs'

it('returns Drawing::Selected.shape as a live projection', () => {
  const drawing = make_selected_drawing(10, 20, 30)
  if (drawing.type !== 'selected') throw new Error('expected Drawing::Selected')
  const projected = drawing.shape

  replace_with_square(projected, 30, 40, 50)

  expect(projected.type).toBe('square')
  expect(drawing.shape.type).toBe('square')
  if (drawing.shape.type !== 'square') throw new Error('expected Shape::Square')
  expect(drawing.shape.layer).toBe(40)
  expect(drawing.shape.center.x).toBe(50)
})

it('returns repeated Drawing::Selected.shape getters for the same Rust field', () => {
  const drawing = make_selected_drawing(10, 20, 30)
  if (drawing.type !== 'selected') throw new Error('expected Drawing::Selected')

  expect(drawing.shape).toBe(drawing.shape)
})

it('clones a Shape assigned to Drawing::Selected.shape', () => {
  const drawing = make_selected_drawing(1, 2, 3)
  const replacement = make_circle(10, 20, 30)
  if (drawing.type !== 'selected') throw new Error('expected Drawing::Selected')

  drawing.shape = replacement
  set_circle_radius(replacement, 99)

  if (drawing.shape.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(drawing.shape.radius).toBe(10)
  if (replacement.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(replacement.radius).toBe(99)
})

it('detaches the old Shape projection when Drawing::Selected.shape is replaced', () => {
  const drawing = make_selected_drawing(1, 2, 3)
  if (drawing.type !== 'selected') throw new Error('expected Drawing::Selected')
  const oldShape = drawing.shape

  drawing.shape = make_circle(10, 20, 30)
  set_circle_radius(oldShape, 99)

  if (oldShape.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(oldShape.radius).toBe(99)
  if (drawing.shape.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(drawing.shape.radius).toBe(10)
})
