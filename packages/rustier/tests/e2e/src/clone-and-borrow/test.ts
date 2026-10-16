import { expect, it } from 'vitest'
import {
  Counter,
  Ledger,
  add_and_return,
  add_owned_first,
  add_owned_last,
  add_projected,
  read_twice,
} from './test.rs'

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

it('returns an independent owned result after cloning an aliased argument', () => {
  const counter = Counter.of({ value: 21 })
  const result = add_and_return(counter, counter)

  expect(counter.value).toBe(42)
  expect(result.value).toBe(42)
  result.value = 99

  expect(counter.value).toBe(42)
})

it('clones a projected child before mutably borrowing its parent', () => {
  const ledger = Ledger.of({ counter: { value: 21 } })
  const child = ledger.counter

  expect(add_projected(ledger, child)).toBe(42)
  expect(ledger.counter.value).toBe(42)
  expect(child.value).toBe(42)
})
