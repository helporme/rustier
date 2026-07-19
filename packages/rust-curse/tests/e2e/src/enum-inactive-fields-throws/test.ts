import { expect, it } from 'vitest'
import { make_third_boundary_enum } from './test.rs'

it('throws when reading or writing a unique field on an inactive variant', () => {
  const value = make_third_boundary_enum()

  expect(() => Reflect.get(value, 'unique')).toThrow(
    'cannot access inactive enum variant field',
  )
  expect(() => Reflect.set(value, 'unique', 10)).toThrow(
    'cannot access inactive enum variant field',
  )
})

it('throws when reading or writing a grouped field unavailable on the active variant', () => {
  const value = make_third_boundary_enum()

  expect(() => Reflect.get(value, 'shared')).toThrow(
    'Field shared is not available on the active BoundaryEnum variant',
  )
  expect(() => Reflect.set(value, 'shared', 10)).toThrow(
    'Field shared is not available on the active BoundaryEnum variant',
  )
})
