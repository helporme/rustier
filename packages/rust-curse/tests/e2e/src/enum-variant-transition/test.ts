import { expect, it } from 'vitest'
import { make_circle, replace_with_square, set_point_x } from './test.rs'

it('keeps the Shape wrapper live when replace_with_square changes Circle to Square', () => {
  const shape = make_circle(10, 20, 30)

  replace_with_square(shape, 60, 40, 50)

  expect(shape.type).toBe('Square')
  if (shape.type !== 'Square') throw new Error('expected Shape::Square')

  expect(shape.layer).toBe(40)
  expect(shape.center.x).toBe(50)

  set_point_x(shape.center, 70)

  expect(shape.center.x).toBe(70)
})
