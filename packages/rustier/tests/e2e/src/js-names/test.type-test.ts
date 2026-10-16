import { expectTypeOf, test } from 'vitest'
import { Header, reserved_parameters } from './test.rs'

test('declares JS-sensitive fields, methods, and escaped parameter names', () => {
  const header = Header.of({ type: 'text/plain', constructor: 42, ['__proto__']: 7 })

  expectTypeOf(header.type).toEqualTypeOf<string>()
  expectTypeOf(header.constructor).toEqualTypeOf<number>()
  header.constructor = 7
  expectTypeOf<Header.Plain>().toEqualTypeOf<{ type: string; constructor: number; __proto__: number }>()
  expectTypeOf<Header.Coerce>().toEqualTypeOf<{ type: string; constructor: number; __proto__: number }>()
  expectTypeOf(header.__proto__).toEqualTypeOf<number>()
  expectTypeOf(header.delete()).toEqualTypeOf<void>()
  expectTypeOf(Header.default()).toEqualTypeOf<Header>()
  expectTypeOf(reserved_parameters(1, 2, 3)).toEqualTypeOf<number>()
})
