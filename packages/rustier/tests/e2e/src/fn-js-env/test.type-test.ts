import { expectTypeOf, test } from 'vitest'
import { accepts_env, add_with_env, same_env } from './test.rs'

test('omits JsEnv parameters from the TypeScript API', () => {
  expectTypeOf(accepts_env).parameters.toEqualTypeOf<[]>()
  expectTypeOf(add_with_env).parameters.toEqualTypeOf<[number, number]>()
  expectTypeOf(same_env).parameters.toEqualTypeOf<[]>()
})
