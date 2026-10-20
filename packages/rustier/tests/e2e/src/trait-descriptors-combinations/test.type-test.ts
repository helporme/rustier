import { expectTypeOf, test } from 'vitest'
import { OtherSeed, Pair, Seed } from './test.rs'

test('accepts only complete discovered combinations, including reference keys', () => {
  expectTypeOf(Pair('u32', 'String').$args).toEqualTypeOf<
    readonly ['u32', 'String']
  >()
  expectTypeOf(Pair(['&', Seed], ['&mut', OtherSeed]).$args).toEqualTypeOf<
    readonly [readonly ['&', typeof Seed], readonly ['&mut', typeof OtherSeed]]
  >()

  // @ts-expect-error undiscovered combination
  Pair('u32', 'u32')
  // @ts-expect-error different class identities
  Pair(['&', OtherSeed], ['&mut', Seed])
  const independent = (left: 'u32' | 'String', right: 'u32' | 'String') => {
    // @ts-expect-error independent unions do not establish a supported combination
    Pair(left, right)
  }
  expectTypeOf(independent).returns.toBeVoid()
})
