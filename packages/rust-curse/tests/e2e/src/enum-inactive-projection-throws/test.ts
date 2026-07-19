import { expect, it } from 'vitest'
import { make_circle, replace_with_square } from './test.rs'

it('throws when accessing the old Circle.center after Shape becomes Square', () => {
  const shape = make_circle(10, 20, 30)
  if (shape.type !== 'circle') throw new Error('expected Shape::Circle')
  const oldCenter = shape.center

  replace_with_square(shape, 30, 40, 50)

  expect(() => oldCenter.x).toThrow('cannot access inactive enum variant field')
  expect(() => {
    oldCenter.x = 60
  }).toThrow('cannot access inactive enum variant field')
})
