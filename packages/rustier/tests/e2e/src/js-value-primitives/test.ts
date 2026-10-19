import { expect, it } from 'vitest'
import { ValueStore } from '../fn-js-value/test.rs'
import {
  local_string,
  make_boolean,
  make_null,
  make_string,
  make_undefined,
  read_boolean,
  read_number,
  read_string,
  string_after_allocation,
  type_name,
} from './test.rs'

const types: [string, unknown][] = [
  ['Undefined', undefined],
  ['Null', null],
  ['Boolean', false],
  ['Boolean', true],
  ['Number', 0],
  ['Number', NaN],
  ['Number', Infinity],
  ['String', ''],
  ['String', 'hello'],
  ['Bigint', 42n],
  ['Symbol', Symbol('value')],
  ['Object', {}],
  ['Object', []],
  ['Object', Object(42)],
  ['Function', () => 42],
]

it.each(types)('reports %s without reading properties', (expected, value) => {
  expect(type_name(value)).toBe(expected)
})

it.each([0, -0, 42.5, Infinity, -Infinity, NaN])(
  'extracts the number %s without losing its value',
  (value) => {
    const result = read_number(value)
    expect(result.is_some()).toBe(true)
    expect(result.unwrap()).toBe(value)
  },
)

it.each([false, true])('extracts the boolean %s', (value) => {
  const result = read_boolean(value)
  expect(result.is_some()).toBe(true)
  expect(result.unwrap()).toBe(value)
  expect(make_boolean(value)).toBe(value)
})

const strings = [
  '',
  'plain text',
  'a\0b\0',
  '\ufeffleading BOM',
  '\ufeff\ufefftwo BOMs',
  'Привет 你好 👩🏽‍💻 e\u0301',
  '🦀'.repeat(10000),
]

it.each(strings)('constructs and extracts a UTF-8 string %#', (value) => {
  expect(make_string(value)).toBe(value)
  const result = read_string(value)
  expect(result.is_some()).toBe(true)
  expect(result.unwrap()).toBe(value)
})

it('replaces unpaired UTF-16 surrogates when extracting a Rust String', () => {
  expect(read_string('\ud800').unwrap()).toBe('\ufffd')
  expect(read_string('\udfff').unwrap()).toBe('\ufffd')
  expect(read_string('\ud800x\udfff').unwrap()).toBe('\ufffdx\ufffd')
})

it('does not coerce primitives or boxed values to another type', () => {
  for (const value of ['42', true, 42n, null, undefined, Object(42)]) {
    expect(read_number(value).is_none()).toBe(true)
  }
  for (const value of [0, 1, 'true', null, undefined, Object(false)]) {
    expect(read_boolean(value).is_none()).toBe(true)
  }
  for (const value of [42, false, 42n, null, undefined, Object('text')]) {
    expect(read_string(value).is_none()).toBe(true)
  }
})

it('does not invoke getters or coercion hooks while inspecting an object', () => {
  const value = new Proxy(
    {},
    {
      get() {
        throw new Error('unexpected property access')
      },
    },
  )

  expect(type_name(value)).toBe('Object')
  expect(read_number(value).is_none()).toBe(true)
  expect(read_boolean(value).is_none()).toBe(true)
  expect(read_string(value).is_none()).toBe(true)
})

it('constructs distinct null and undefined values', () => {
  expect(make_null()).toBeNull()
  expect(make_undefined()).toBeUndefined()
  expect(type_name(make_null())).toBe('Null')
  expect(type_name(make_undefined())).toBe('Undefined')
})

it('can create and inspect strings without JsValue in the exported signature', () => {
  expect(local_string()).toBe('\ufeffready\0🦀')
})

it('reads a retained string after a large Rust allocation', () => {
  const text = '\ufeffbefore\0growth🦀'.repeat(1000)
  expect(string_after_allocation(text)).toBe(text)
  expect(make_string('after')).toBe('after')
  expect(read_string('after').unwrap()).toBe('after')
})

it('inspects cloned values after the original Rust owner is cleared', () => {
  const store = new ValueStore(make_string('retained\0🦀'))
  const copy = store.get()

  store.clear()
  expect(type_name(copy)).toBe('String')
  expect(read_string(copy).unwrap()).toBe('retained\0🦀')
})
