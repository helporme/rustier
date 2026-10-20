import { expectTypeOf, test } from 'vitest'
import { From } from '$rust/core/convert'
import { Choice, Seed, Wrap } from './keys.rs'
import { Seed as OtherSeed } from './other_keys.rs'

test('preserves constructor types for ordinary, enum, and nested generic keys', () => {
  expectTypeOf(From(Seed).$args).toEqualTypeOf<readonly [typeof Seed]>()
  expectTypeOf(From(OtherSeed).$args).toEqualTypeOf<
    readonly [typeof OtherSeed]
  >()
  expectTypeOf(From(Choice).$args).toEqualTypeOf<readonly [typeof Choice]>()

  const Numbers = Wrap('u32')
  const Seeds = Wrap(Seed)
  const Nested = Wrap(Numbers)
  expectTypeOf(From(Numbers).$args).toEqualTypeOf<readonly [typeof Numbers]>()
  expectTypeOf(From(Seeds).$args).toEqualTypeOf<readonly [typeof Seeds]>()
  expectTypeOf(From(Nested).$args).toEqualTypeOf<readonly [typeof Nested]>()
})
