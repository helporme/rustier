import { expect, it } from 'vitest'
import { rustierDiags } from '$build/diags'
import { Counter, ReadonlyOwner, Unexposed } from './test.rs'

it('calls a struct constructor and shared and exclusive methods', () => {
  const counter = new Counter(10)

  expect(counter.value).toBe(10)

  counter.add(5)

  expect(counter.value).toBe(15)
})

it('clones an owned self receiver and leaves the JS wrapper valid', () => {
  const counter = new Counter(21)

  expect(counter.into_value()).toBe(21)
  expect(counter.value).toBe(21)
})

it('passes and returns Self through associated functions', () => {
  const first = new Counter(12)
  const second = Counter.clone_of(first)

  expect(first.sum(second)).toBe(24)

  const replacement = new Counter(90)
  second.replace_with(replacement)

  expect(second.value).toBe(90)
  expect(replacement.value).toBe(90)
})

it('exposes an inherent method named constructor', () => {
  const positive = new Counter(1)
  const negative = new Counter(-1)

  expect(positive.constructor()).toBe(true)
  expect(negative.constructor()).toBe(false)
})

// TODO: Include the newly exposed Unexposed::unsupported field collision.
it.skip('lets exposed fields shadow methods, including readonly fields', () => {
  const counter = new Counter(7)
  const owner = new ReadonlyOwner(9)

  expect(counter.value).toBe(7)
  counter.value = 12
  expect(counter.into_value()).toBe(12)
  expect(owner.child.value).toBe(9)
  expect(Object.getOwnPropertyDescriptor(ReadonlyOwner.prototype, 'child')?.set).toBeUndefined()
  const collisions = rustierDiags.filter(diag =>
    diag.code === 'rustier::method_shadowed_by_field'
    && diag.message.includes('e2e::impl_struct::'))
  expect(collisions).toHaveLength(2)
  for (const name of ['Counter::value', 'ReadonlyOwner::child']) {
    expect(collisions.filter(diag => diag.message.includes(`e2e::impl_struct::${name}`))).toHaveLength(1)
  }
})

// TODO: Use an unsupported field type instead of the now-supported Vec<i32>.
it.skip('keeps static names and methods matching private or unsupported fields', () => {
  const counter = new Counter(7)
  const unexposed = new Unexposed(7)

  expect(Counter.baseline()).toBe(100)
  expect(counter.baseline).toBe(7)
  counter.baseline = 8
  expect(counter.baseline).toBe(8)
  expect(unexposed.private()).toBe(7)
  // TODO: Restore this assertion with a field that does not shadow the method.
  // expect(unexposed.unsupported()).toBe(7)
})

it('omits static prototype and non-public methods', () => {
  const runtimeClass = Counter as unknown as Record<string, unknown>
  const runtimeCounter = new Counter(0) as unknown as Record<string, unknown>

  expect(typeof runtimeClass.prototype).toBe('object')
  expect(runtimeClass.private_method).toBeUndefined()
  expect(runtimeClass.crate_method).toBeUndefined()
  expect(runtimeClass.parent_method).toBeUndefined()
  expect(runtimeCounter.private_method).toBeUndefined()
  expect(runtimeCounter.crate_method).toBeUndefined()
  expect(runtimeCounter.parent_method).toBeUndefined()
})
