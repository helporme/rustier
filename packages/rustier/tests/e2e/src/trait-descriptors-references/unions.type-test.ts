import { expectTypeOf, test } from 'vitest'
import { From } from '$rust/core/convert'
import { OtherSeed, Seed } from './test.rs'

test('freezes reference members of mixed primitive and tuple unions', () => {
  const normalize = (key: 'u32' | ['&', typeof Seed]) => {
    const descriptor = From(key)
    const argument = descriptor.$args[0]
    if (typeof argument !== 'string') {
      // @ts-expect-error a union member remains readonly after narrowing
      argument[0] = '&'
    }
    return descriptor.$args
  }
  expectTypeOf(normalize).returns.toEqualTypeOf<
    readonly ['u32' | readonly ['&', typeof Seed]]
  >()
})

test('preserves alternatives in a union of reference prefixes', () => {
  const normalize = (key: ['&', typeof Seed] | ['&mut', typeof Seed]) =>
    From(key).$args
  expectTypeOf(normalize).returns.toEqualTypeOf<
    readonly [readonly ['&', typeof Seed] | readonly ['&mut', typeof Seed]]
  >()
})

test('preserves the correlation between reference prefixes and constructors', () => {
  const normalize = (key: ['&', typeof Seed] | ['&mut', typeof OtherSeed]) =>
    From(key).$args
  expectTypeOf(normalize).returns.toEqualTypeOf<
    readonly [readonly ['&', typeof Seed] | readonly ['&mut', typeof OtherSeed]]
  >()
})
