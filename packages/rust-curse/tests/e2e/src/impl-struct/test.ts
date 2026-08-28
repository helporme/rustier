import { expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import { Counter } from './test.rs'

it('calls a struct constructor and shared and exclusive methods', () => {
  const counter = new Counter(10)

  expect(counter.value()).toBe(10)

  counter.add(5)

  expect(counter.value()).toBe(15)
})

it('clones an owned self receiver and leaves the JS wrapper valid', () => {
  const counter = new Counter(21)

  expect(counter.into_value()).toBe(21)
  expect(counter.value()).toBe(21)
})

it('passes and returns Self through associated functions', () => {
  const first = new Counter(12)
  const second = Counter.clone_of(first)

  expect(first.sum(second)).toBe(24)

  const replacement = new Counter(90)
  second.replace_with(replacement)

  expect(second.value()).toBe(90)
  expect(replacement.value()).toBe(90)
})

it('exposes an inherent method named constructor', () => {
  const positive = new Counter(1)
  const negative = new Counter(-1)

  expect(positive.constructor()).toBe(true)
  expect(negative.constructor()).toBe(false)
})

it('lets the method shadow a field with the same name', () => {
  const counter = new Counter(7)
  const runtimeCounter = counter as unknown as Record<string, unknown>

  expect(counter.value()).toBe(7)
  expect(typeof runtimeCounter.value).toBe('function')
  expect(rustCurseDiags.join('\n')).toContain(
    'rust_curse::field_shadowed_by_method',
  )
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
