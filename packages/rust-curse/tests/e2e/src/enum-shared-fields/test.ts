import { expect, it } from 'vitest'
import {
  make_circle,
  make_number_value,
  make_square,
  make_text_value,
} from './test.rs'

it('reads and writes Shape.layer on Circle and Square', () => {
  const circle = make_circle(10, 20, 30)
  const square = make_square(50, 40, 60)
  if (circle.type !== 'circle') throw new Error('expected Shape::Circle')
  if (square.type !== 'square') throw new Error('expected Shape::Square')

  expect(circle.layer).toBe(20)
  expect(square.layer).toBe(40)

  circle.layer = 60
  square.layer = 70

  expect(circle.layer).toBe(60)
  expect(square.layer).toBe(70)
})

it('reads and writes FormValue.value as i32 for Number and String for Text', () => {
  const number = make_number_value(10)
  const text = make_text_value('initial 🦀')
  if (number.type !== 'number') throw new Error('expected FormValue::Number')
  if (text.type !== 'text') throw new Error('expected FormValue::Text')

  expect(number.value).toBe(10)
  expect(text.value).toBe('initial 🦀')

  number.value = 20
  text.value = 'updated 🦀'

  expect(number.value).toBe(20)
  expect(text.value).toBe('updated 🦀')
})
