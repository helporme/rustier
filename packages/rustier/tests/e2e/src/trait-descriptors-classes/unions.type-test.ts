import { expectTypeOf, test } from 'vitest'
import { From } from '$rust/core/convert'
import { Seed, Wrap } from './keys.rs'
import { Seed as OtherSeed } from './other_keys.rs'

test('preserves constructor variables and same-named constructor unions', () => {
  const single = (key: typeof Seed) => From(key).$args
  const union = (key: typeof Seed | typeof OtherSeed) => From(key).$args
  expectTypeOf(single).returns.toEqualTypeOf<readonly [typeof Seed]>()
  expectTypeOf(union).returns.toEqualTypeOf<
    readonly [typeof Seed | typeof OtherSeed]
  >()
})

test('preserves instantiated generic constructor unions without widening', () => {
  const Numbers = Wrap('u32')
  const Seeds = Wrap(Seed)
  const normalize = (key: typeof Numbers | typeof Seeds) => From(key).$args
  expectTypeOf(normalize).returns.toEqualTypeOf<
    readonly [typeof Numbers | typeof Seeds]
  >()
})
