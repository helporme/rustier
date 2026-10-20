import { expect, it } from 'vitest'
import { From } from '$rust/core/convert'
import { OtherSeed, Seed } from './test.rs'

it('matches fresh reference tuples by their contents and copies them into immutable descriptors', () => {
  const input: ['&', typeof Seed] = ['&', Seed]
  const descriptor = From(input)

  expect(descriptor).toEqual(From(['&', Seed]))
  expect(descriptor.$args[0]).not.toBe(input)
  expect(From(['&mut', Seed]).$args).toEqual([['&mut', Seed]])
  expect(Object.isFrozen(descriptor)).toBe(true)
  expect(Object.isFrozen(descriptor.$args)).toBe(true)
  expect(Object.isFrozen(descriptor.$args[0])).toBe(true)
  Reflect.set(input, 0, '&mut')
  Reflect.set(input, 1, OtherSeed)
  expect(descriptor.$args).toEqual([['&', Seed]])
  expect(Reflect.set(descriptor.$args[0], 0, '&mut')).toBe(false)
})

it('rejects instances and malformed external reference keys', () => {
  for (const args of [
    [Seed.of({ value: 1 })],
    [['&', Seed, 'extra']],
    [['mut', Seed]],
    [['&', 'str']],
    [{ prefix: '&', class: Seed }],
  ]) {
    expect(() => Reflect.apply(From, undefined, args)).toThrow(TypeError)
  }
})
