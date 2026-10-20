import { expectTypeOf, test } from 'vitest'
import * as bindings from './test.rs'

test('omits deprecated callables from declarations', () => {
  expectTypeOf(bindings).not.toHaveProperty('old_function')
  expectTypeOf(bindings.Example).not.toHaveProperty('new')
  expectTypeOf(bindings.Example).not.toHaveProperty('old_static')
  expectTypeOf<bindings.Example>().not.toHaveProperty('old')
  expectTypeOf(bindings.Example.create(7).hidden()).toEqualTypeOf<number>()
})
