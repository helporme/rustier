import { expect, it } from 'vitest'
import { Counter, set_after_check } from '../clone-and-borrow/test.rs'
import {
  copy_counter_value,
  make_counter_struct,
  set_two_counter_values,
} from '../struct-bad-alias-throws/test.rs'

it('releases a mutable borrow after a later argument fails numeric conversion', () => {
  const counter = Counter.of({ value: 10 })

  expect(() => set_after_check(counter, 128)).toThrow('cannot cast 128 to i8')
  expect(counter.value).toBe(10)
  set_after_check(counter, 42)

  expect(counter.value).toBe(42)
})

it('releases both shared and exclusive borrows after an alias error', () => {
  const first = make_counter_struct(10)
  const second = make_counter_struct(20)

  expect(() => copy_counter_value(first, first)).toThrow()
  expect(first.value).toBe(10)
  copy_counter_value(first, second)
  expect(second.value).toBe(10)

  expect(() => set_two_counter_values(first, first)).toThrow()
  expect(first.value).toBe(10)
  set_two_counter_values(first, second)

  expect(first.value).toBe(1)
  expect(second.value).toBe(2)
})
