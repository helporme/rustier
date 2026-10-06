import { expect, it } from 'vitest'
import {
  make_canvas,
  make_rgb_color,
  make_selected_drawing,
  read_shape_layer,
} from './test.rs'

it('throws when actual Color::Rgb is passed to a function expecting Shape', () => {
  const color = make_rgb_color(10, 20, 30)

  expect(() => read_shape_layer(color as never)).toThrow()
})

it('throws when actual Color::Rgb is assigned to Canvas.shape of expected type Shape', () => {
  const canvas = make_canvas(10, 20, 30)
  const color = make_rgb_color(40, 50, 60)

  expect(() => {
    canvas.shape = color as never
  }).toThrow()
})

it('throws when actual Color::Rgb is assigned to Drawing::Selected.shape of expected type Shape', () => {
  const drawing = make_selected_drawing(10, 20, 30)
  const color = make_rgb_color(40, 50, 60)
  if (drawing.type !== 'Selected') throw new Error('expected Drawing::Selected')

  expect(() => {
    drawing.shape = color as never
  }).toThrow()
})
