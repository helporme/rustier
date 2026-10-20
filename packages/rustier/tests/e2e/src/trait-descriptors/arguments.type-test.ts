import { expectTypeOf, test } from 'vitest'
import { From } from '$rust/core/convert'

test('preserves literal and union-valued primitive variables', () => {
  const literal = (key: 'u32') => From(key).$args
  const union = (key: 'u32' | '&str') => From(key).$args
  expectTypeOf(literal).returns.toEqualTypeOf<readonly ['u32']>()
  expectTypeOf(union).returns.toEqualTypeOf<readonly ['u32' | '&str']>()
})

test('normalizes mutable and readonly argument tuples', () => {
  const mutable = (args: ['u32']) => From(...args).$args
  const immutable = (args: readonly ['u32']) => From(...args).$args
  const union = (args: ['u32'] | ['&str']) => From(...args).$args
  expectTypeOf(mutable).returns.toEqualTypeOf<readonly ['u32']>()
  expectTypeOf(immutable).returns.toEqualTypeOf<readonly ['u32']>()
  expectTypeOf(union).returns.toEqualTypeOf<
    readonly ['u32'] | readonly ['&str']
  >()
})

test('rejects widened primitive variables and unions containing unsupported keys', () => {
  const widened = (key: string) => {
    // @ts-expect-error a widened string is not a discovered type key
    From(key)
  }
  const unsupported = (key: 'u32' | 'u64') => {
    // @ts-expect-error every union member must be supported
    From(key)
  }
  expectTypeOf(widened).returns.toBeVoid()
  expectTypeOf(unsupported).returns.toBeVoid()
})
