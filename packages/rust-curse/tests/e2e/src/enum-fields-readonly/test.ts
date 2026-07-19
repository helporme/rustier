import { expect, it } from 'vitest'
import {
  make_child_struct,
  make_readonly_mixed_readonly_enum,
  make_writable_mixed_readonly_enum,
  set_non_clone_child_value,
} from './test.rs'

it('writes a common field when it is writable on the active enum variant', () => {
  const value = make_writable_mixed_readonly_enum(10)
  const replacement = make_child_struct(20)
  if (value.type !== 'writable') throw new Error('expected the writable variant')

  value.item = replacement
  replacement.value = 30

  expect(value.item.value).toBe(20)
})

it('projects a common field when it is readonly on the active enum variant', () => {
  const value = make_readonly_mixed_readonly_enum(10)
  if (value.type !== 'readonly') throw new Error('expected the readonly variant')

  set_non_clone_child_value(value.item, 20)

  expect(value.item.value).toBe(20)
})
