import { expect, it } from 'vitest'
import { Seed } from './keys.rs'
import {
  default as DefaultTrait,
  type as TypeTrait,
  Object as ObjectTrait,
  Array as ArrayTrait,
  TypeError as TypeErrorTrait,
  globalThis as GlobalThisTrait,
  Symbol as SymbolTrait,
  Local,
} from './keywords.rs'

it('exports raw and JS-reserved trait names without shadowing globals', () => {
  expect(DefaultTrait.$args).toEqual([])
  expect(TypeTrait('u32').$args).toEqual(['u32'])
  expect(ObjectTrait.$args).toEqual([])
  expect(SymbolTrait.$args).toEqual([])
  expect(ArrayTrait(['&', Seed]).$args).toEqual([['&', Seed]])
  expect(TypeErrorTrait('u32').$args).toEqual(['u32'])
  expect(GlobalThisTrait('u32').$args).toEqual(['u32'])
  expect(Local.of({ 0: 7 })[0]).toBe(7)
  expect(() => Reflect.apply(TypeErrorTrait, undefined, ['u64'])).toThrow(
    TypeError,
  )
})
