import { expect, it } from 'vitest'
import {
  make_first_boundary_enum,
  replace_boundary_enum_with_second,
  set_child_value,
} from './test.rs'

it('keeps the enum wrapper live when Rust replaces its active variant', () => {
  const value = make_first_boundary_enum(10, 20, 30)

  replace_boundary_enum_with_second(value, 40, 50)

  expect(value.type).toBe('second')
  if (value.type !== 'second') throw new Error('expected the second variant')

  expect(value.shared).toBe(40)
  expect(value.child.value).toBe(50)

  set_child_value(value.child, 70)

  expect(value.child.value).toBe(70)
})
