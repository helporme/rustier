import { expect, it } from 'vitest'
import { Counter, add_owned_first, add_owned_last, read_twice } from './test.rs'

it('accepts two shared references to the same Rust allocation', () => {
  const counter = Counter.of({ value: 21 })

  expect(read_twice(counter, counter)).toBe(42)
  counter.value = 7
  expect(read_twice(counter, counter)).toBe(14)
})

it('clones an owned argument before taking a mutable reference to the same value', () => {
  const counter = Counter.of({ value: 21 })

  add_owned_first(counter, counter)

  expect(counter.value).toBe(42)
})

it('clones an owned argument even when the mutable reference is the first parameter', () => {
  const counter = Counter.of({ value: 21 })

  add_owned_last(counter, counter)

  expect(counter.value).toBe(42)
})

it('clones an owned method argument that is also its mutable receiver', () => {
  const counter = Counter.of({ value: 21 })

  counter.add(counter)

  expect(counter.value).toBe(42)
})
