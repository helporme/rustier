import { expect, it } from 'vitest'
import { make_hidden } from './test.rs'

it('throws when reading or writing Circle.radius while Shape is Hidden', () => {
  const shape = make_hidden()

  expect(() => Reflect.get(shape, 'radius')).toThrow(
    'cannot access inactive enum variant field',
  )
  expect(() => Reflect.set(shape, 'radius', 10)).toThrow(
    'cannot access inactive enum variant field',
  )
})

it('throws when reading or writing Shape.layer while Shape is Hidden', () => {
  const shape = make_hidden()

  expect(() => Reflect.get(shape, 'layer')).toThrow(
    'Field layer is not available on the active Shape variant',
  )
  expect(() => Reflect.set(shape, 'layer', 10)).toThrow(
    'Field layer is not available on the active Shape variant',
  )
})
