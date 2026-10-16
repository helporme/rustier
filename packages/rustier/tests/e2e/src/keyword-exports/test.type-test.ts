import { expectTypeOf, test } from 'vitest'
import defaultFunction, {
  arguments as argumentsFunction,
  await as awaitFunction,
  class as classFunction,
  default_,
  delete as deleteFunction,
  eval as evalFunction,
  interface as interfaceFunction,
  yield as yieldFunction,
} from './test.rs'
import * as bindings from './test.rs'

test('declares keyword exports through default, aliased, and namespace imports', () => {
  expectTypeOf(defaultFunction).toEqualTypeOf<(value: number) => number>()
  expectTypeOf(bindings.default).toEqualTypeOf<typeof defaultFunction>()
  expectTypeOf(bindings.delete).toEqualTypeOf<typeof deleteFunction>()
  expectTypeOf(bindings.await).toEqualTypeOf<typeof awaitFunction>()
  expectTypeOf(bindings.arguments).toEqualTypeOf<typeof argumentsFunction>()
  expectTypeOf(bindings.eval).toEqualTypeOf<typeof evalFunction>()
  expectTypeOf(bindings.yield).toEqualTypeOf<typeof yieldFunction>()
  expectTypeOf(bindings.interface).toEqualTypeOf<typeof interfaceFunction>()
  expectTypeOf(bindings.class).toEqualTypeOf<typeof classFunction>()
  expectTypeOf(default_(1)).toEqualTypeOf<number>()
})
