import { expectTypeOf, test } from 'vitest'
import * as bindings from './test.rs'

test('omits unsupported impl items from declarations', () => {
  const widget = bindings.Widget.new(1)

  expectTypeOf(bindings).not.toHaveProperty('Generic')
  expectTypeOf(bindings.Widget).not.toHaveProperty('DEFAULT_ID')
  expectTypeOf(bindings.Widget).not.toHaveProperty('KIND')
  expectTypeOf(widget).not.toHaveProperty('identity')
  expectTypeOf(widget).not.toHaveProperty('blanket')
  expectTypeOf(widget).not.toHaveProperty('marked')
  expectTypeOf(widget).not.toHaveProperty('generic')
  expectTypeOf(widget).not.toHaveProperty('dangerous')
  expectTypeOf(widget).not.toHaveProperty('asynchronous')
})
