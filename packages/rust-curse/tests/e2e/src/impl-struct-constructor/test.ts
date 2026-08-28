import { expect, it } from 'vitest'
import { make_receiver_new, NamedReturn, NoArgs, WrongReturn } from './test.rs'

it('maps zero-argument new returning Self to a constructor', () => {
  const value = new NoArgs()

  expect(value.works()).toBe(true)
  expect((NoArgs as unknown as Record<string, unknown>).new).toBeUndefined()
})

it('maps argument-taking new with a named self return to a constructor', () => {
  const value = new NamedReturn(42)

  expect(value.value()).toBe(42)
  expect(
    (NamedReturn as unknown as Record<string, unknown>).new,
  ).toBeUndefined()
})

it('keeps a receiver-taking new as an instance method', () => {
  const value = make_receiver_new(5).new()

  expect(value.value()).toBe(6)
})

it('keeps new returning a different class as a static method', () => {
  const value = WrongReturn.new(9)
  const RuntimeWrongReturn = WrongReturn as unknown as new () => WrongReturn

  expect(value.value()).toBe(9)
  expect(() => new RuntimeWrongReturn()).toThrow(TypeError)
})
