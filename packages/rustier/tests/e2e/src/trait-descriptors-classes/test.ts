import { expect, it } from 'vitest'
import { From } from '$rust/core/convert'
import { Choice, Seed, Wrap } from './keys.rs'
import { Seed as OtherSeed } from './other_keys.rs'

it('preserves identity for same-named classes, enum classes, and generic classes', () => {
  expect(Seed.name).toBe(OtherSeed.name)
  expect(From(Seed).$args[0]).toBe(Seed)
  expect(From(OtherSeed).$args[0]).toBe(OtherSeed)
  expect(From(Choice).$args[0]).toBe(Choice)
  expect(From(Wrap('u32')).$args[0]).toBe(Wrap('u32'))
  expect(From(Wrap(Seed)).$args[0]).toBe(Wrap(Seed))
  expect(From(Wrap(Wrap('u32'))).$args[0]).toBe(Wrap(Wrap('u32')))
})
