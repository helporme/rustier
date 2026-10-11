import { expect, it } from 'vitest'
import { make_receiver_new, NamedReturn, NoArgs, WrongReturn } from './test.rs'

it('exposes zero-argument new returning Self as a constructor and static method', () => {
  const value = new NoArgs()

  expect(value.works()).toBe(true)
  expect(NoArgs.new().works()).toBe(true)
})

it('exposes argument-taking new with a named self return as a constructor and static method', () => {
  const value = new NamedReturn(42)

  expect(value.value()).toBe(42)
  expect(NamedReturn.new(21).value()).toBe(21)
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
