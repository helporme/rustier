import { expect, it } from 'vitest'
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

it.each([
  ['default', defaultFunction, 1],
  ['delete', deleteFunction, 2],
  ['await', awaitFunction, 3],
  ['arguments', argumentsFunction, 4],
  ['eval', evalFunction, 5],
  ['yield', yieldFunction, 6],
  ['interface', interfaceFunction, 7],
  ['class', classFunction, 8],
] as const)('exports the Rust function %s under its original name', (name, fn, increment) => {
  expect(fn(10)).toBe(10 + increment)
  expect(bindings[name]).toBe(fn)
})

it('keeps a similarly named ordinary export distinct from a keyword export', () => {
  expect(default_(10)).toBe(19)
  expect(default_(10)).not.toBe(defaultFunction(10))
  expect(Object.keys(bindings).sort()).toEqual([
    'arguments', 'await', 'class', 'default', 'default_', 'delete', 'eval', 'interface', 'yield',
  ])
})
