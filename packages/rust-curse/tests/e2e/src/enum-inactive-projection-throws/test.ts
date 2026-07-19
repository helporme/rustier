import { expect, it } from 'vitest'
import {
  make_first_boundary_enum,
  replace_boundary_enum_with_second,
} from './test.rs'

it('throws when using a projected field after its enum variant becomes inactive', () => {
  const value = make_first_boundary_enum(10, 20, 30)
  if (value.type !== 'first') throw new Error('expected the first variant')
  const oldChild = value.child

  replace_boundary_enum_with_second(value, 40, 50)

  expect(() => oldChild.value).toThrow(
    'cannot access inactive enum variant field',
  )
  expect(() => {
    oldChild.value = 60
  }).toThrow('cannot access inactive enum variant field')
})
