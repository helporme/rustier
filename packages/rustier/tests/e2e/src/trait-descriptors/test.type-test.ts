import { expectTypeOf, test } from 'vitest'
import { Clone } from '$rust/core/clone'
import { From } from '$rust/core/convert'
import { Convert, Probe, Read } from './contracts.rs'
import { Read as OtherRead } from './other_contracts.rs'

test('preserves literal argument tuples and distinct trait identities', () => {
  expectTypeOf(Clone.$args).toEqualTypeOf<readonly []>()
  expectTypeOf(From('u32').$args).toEqualTypeOf<readonly ['u32']>()
  expectTypeOf(From('&str').$args).toEqualTypeOf<readonly ['&str']>()
  expectTypeOf(Convert('u32').$args).toEqualTypeOf<readonly ['u32']>()
  expectTypeOf(Probe('str').$args).toEqualTypeOf<readonly ['str']>()
  expectTypeOf(Read).not.toEqualTypeOf<typeof OtherRead>()
})

test('rejects unavailable keys and incorrect descriptor calls', () => {
  // @ts-expect-error unavailable primitive
  From('u64')
  // @ts-expect-error descriptor arity
  From()
  // @ts-expect-error unavailable custom specialization
  Convert('f64')
  // @ts-expect-error non-generic traits are descriptor constants
  Read()
})
