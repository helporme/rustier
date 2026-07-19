import { expect, it } from 'vitest'
import {
  make_child_struct,
  make_readonly_mixed_readonly_enum,
} from './test.rs'

it('throws when replacing a common field that is readonly on the active enum variant', () => {
  const value = make_readonly_mixed_readonly_enum(10)

  expect(() => Reflect.set(value, 'item', make_child_struct(30))).toThrow(
    'Field is readonly on the active enum variant',
  )
})
