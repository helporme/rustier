import { expectTypeOf, test } from 'vitest'
import { Pair } from './test.rs'

test('normalizes mutable and readonly multi-argument tuples', () => {
  const mutable = (args: ['u32', 'String']) => Pair(...args).$args
  const immutable = (args: readonly ['u32', 'String']) => Pair(...args).$args
  expectTypeOf(mutable).returns.toEqualTypeOf<readonly ['u32', 'String']>()
  expectTypeOf(immutable).returns.toEqualTypeOf<readonly ['u32', 'String']>()
})

test('preserves correlated unions of complete argument tuples', () => {
  const normalize = (args: ['u32', 'String'] | readonly ['String', 'u32']) =>
    Pair(...args).$args
  expectTypeOf(normalize).returns.toEqualTypeOf<
    readonly ['u32', 'String'] | readonly ['String', 'u32']
  >()
})

test('rejects tuples of independent unions and unions with unsupported combinations', () => {
  const independent = (args: ['u32' | 'String', 'u32' | 'String']) => {
    // @ts-expect-error independent elements do not establish a supported combination
    Pair(...args)
  }
  const unsupported = (args: ['u32', 'String'] | ['u32', 'u32']) => {
    // @ts-expect-error every complete tuple in the union must be supported
    Pair(...args)
  }
  expectTypeOf(independent).returns.toBeVoid()
  expectTypeOf(unsupported).returns.toBeVoid()
})
