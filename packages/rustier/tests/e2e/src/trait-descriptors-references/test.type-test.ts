import { expectTypeOf, test } from 'vitest'
import { From } from '$rust/core/convert'
import { OtherSeed, Seed } from './test.rs'

test('types reference tuples without as const and rejects invalid targets', () => {
  const input: ['&', typeof Seed] = ['&', Seed]
  expectTypeOf(From(input).$args).toEqualTypeOf<
    readonly [readonly ['&', typeof Seed]]
  >()
  expectTypeOf(From(['&mut', Seed]).$args).toEqualTypeOf<
    readonly [readonly ['&mut', typeof Seed]]
  >()

  // @ts-expect-error instances are not type keys
  From(Seed.of({ value: 1 }))
  // @ts-expect-error reference strings are reserved for built-in keys
  From('&Seed')
  // @ts-expect-error wrong reference target
  From(['&', OtherSeed])
  // @ts-expect-error reference tuples contain exactly two entries
  From(['&', Seed, Seed])
})

test('returns readonly descriptor data even for mutable tuple inputs', () => {
  const input: ['&', typeof Seed] = ['&', Seed]
  const descriptor = From(input)

  // @ts-expect-error immutable descriptor
  descriptor.$trait = descriptor.$trait
  // @ts-expect-error immutable outer tuple
  descriptor.$args[0] = input
  // @ts-expect-error immutable reference tuple
  descriptor.$args[0][0] = '&'
})
