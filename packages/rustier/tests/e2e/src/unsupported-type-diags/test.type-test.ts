import { expectTypeOf, test } from 'vitest'
import * as bindings from './test.rs'

test('omits unsupported declarations and signatures', () => {
  expectTypeOf(bindings.supported()).toEqualTypeOf<number>()
  expectTypeOf(bindings).not.toHaveProperty('Text')
  expectTypeOf(bindings).not.toHaveProperty('length')
  expectTypeOf(bindings).not.toHaveProperty('Token')
  expectTypeOf(bindings).not.toHaveProperty('make')
})
