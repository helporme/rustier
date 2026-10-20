import { expect, it } from 'vitest'
import { OtherSeed, Pair, Seed } from './test.rs'

it('validates complete combinations rather than independent argument unions', () => {
  expect(Pair('u32', 'String').$args).toEqual(['u32', 'String'])
  expect(Pair('String', 'u32').$args).toEqual(['String', 'u32'])
  expect(Pair(['&', Seed], ['&mut', OtherSeed]).$args).toEqual([
    ['&', Seed],
    ['&mut', OtherSeed],
  ])

  for (const args of [
    ['u32', 'u32'],
    ['String', 'String'],
    [
      ['&', OtherSeed],
      ['&mut', Seed],
    ],
    [
      ['&&mut', Seed],
      ['&mut', OtherSeed],
    ],
  ]) {
    expect(() => Reflect.apply(Pair, undefined, args)).toThrow(TypeError)
  }
})
