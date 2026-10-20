import { expectTypeOf, test } from 'vitest'
import { default as DefaultTrait, type as TypeTrait } from './keywords.rs'

test('preserves descriptor types for raw and reserved names', () => {
  expectTypeOf(DefaultTrait.$args).toEqualTypeOf<readonly []>()
  expectTypeOf(TypeTrait('u32').$args).toEqualTypeOf<readonly ['u32']>()
})
