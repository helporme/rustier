import { expect, it } from 'vitest'
import {
  copy_counter_value,
  describe_owned_and_ref_value_struct,
  describe_two_owned_value_structs,
  make_counter_struct,
  make_value_struct,
  mutate_owned_value_struct,
  read_counter_ref,
  same_counter_refs,
  set_counter_value,
  value_struct_value,
} from './test.rs'

it('returns a struct that can be read, mutated, and passed back to Rust', () => {
  const value = make_value_struct(42)

  expect(value.value).toBe(42)

  value.value = 64

  expect(value.value).toBe(64)
  expect(value_struct_value(value)).toBe(64)
})

it('passes a struct by shared reference without cloning it between calls', () => {
  const value = make_counter_struct(10)

  expect(read_counter_ref(value)).toBe(1)
  expect(read_counter_ref(value)).toBe(2)
  expect(read_counter_ref(value)).toBe(3)
})

it('passes the same struct as two shared references without cloning it', () => {
  const value = make_counter_struct(10)

  expect(same_counter_refs(value, value)).toBe(true)
  expect(same_counter_refs(value, value)).toBe(true)
})

it('passes distinct structs as distinct shared references', () => {
  const first = make_counter_struct(10)
  const second = make_counter_struct(10)

  expect(same_counter_refs(first, second)).toBe(false)
})

it('passes a struct by mutable reference and reflects mutations in JS', () => {
  const value = make_counter_struct(10)

  set_counter_value(value, 99)

  expect(value.value).toBe(99)
})

it('allows shared and mutable references to distinct structs in the same call', () => {
  const source = make_counter_struct(15)
  const target = make_counter_struct(0)

  copy_counter_value(source, target)

  expect(source.value).toBe(15)
  expect(target.value).toBe(15)
})

it('moves a struct by cloning and leaves the original JS wrapper unchanged', () => {
  const original = make_value_struct(10)
  const moved = mutate_owned_value_struct(original, 99)

  expect(original.value).toBe(10)
  expect(moved.value).toBe(99)
})

it('clones the same JS struct independently for two owned arguments', () => {
  const original = make_value_struct(1)

  expect(describe_two_owned_value_structs(original, original)).toBe('11:101')
  expect(original.value).toBe(1)
})

it('allows the same JS struct as an owned argument and a shared reference', () => {
  const original = make_value_struct(1)

  expect(describe_owned_and_ref_value_struct(original, original)).toBe('11:1')
  expect(original.value).toBe(1)
})
