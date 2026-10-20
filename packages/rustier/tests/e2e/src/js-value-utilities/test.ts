import { expect, it } from 'vitest'
import { grow_memory } from '../js-value-properties/test.rs'
import {
  call,
  construct,
  copy_properties,
  define,
  delete as deleteProperty,
  descriptor,
  get,
  get_index,
  get_number,
  has,
  make_symbol,
  own_keys as raw_own_keys,
  read_string,
  set,
  set_number,
  soft_eq,
  strict_eq,
} from './test.rs'

function own_keys(value: unknown): (string | symbol)[] {
  return Array.from(raw_own_keys(value) as ArrayLike<string | symbol>)
}

const values = [
  '',
  'text',
  0,
  -0,
  NaN,
  Infinity,
  42,
  42n,
  false,
  true,
  Symbol('value'),
  undefined,
  null,
  {},
  [],
  () => 42,
]

it('matches strict equality and Object.is without coercion', () => {
  const object = new Proxy(
    {},
    {
      get() {
        throw new Error('unexpected coercion')
      },
    },
  )
  const symbol = Symbol('same')
  const candidates = [...values, object, object, symbol, symbol, Symbol('same')]
  for (const left of candidates) {
    for (const right of candidates) {
      expect(strict_eq(left, right)).toBe(left === right)
      expect(soft_eq(left, right)).toBe(Object.is(left, right))
    }
  }
})

it('copies strings but does not coerce other values', () => {
  expect(read_string('\ufefftext\0🦀')).toBe('\ufefftext\0🦀')
  expect(read_string('\ud800')).toBe('\ufffd')
  expect(
    read_string({
      toString() {
        throw new Error('unexpected coercion')
      },
    }),
  ).toBeNull()
  expect(read_string(42)).toBeNull()
})

it('does not call a replaced Object.is when comparing values', () => {
  const original = Object.is
  let results: boolean[]
  try {
    Object.is = () => {
      throw new Error('unexpected Object.is call')
    }
    results = [soft_eq(NaN, NaN), soft_eq(0, -0), soft_eq(42, 42)]
  } finally {
    Object.is = original
  }
  expect(results).toEqual([true, false, true])
})

it('creates unique symbols with optional descriptions', () => {
  for (const description of [null, '', '\ufeffname\0🦀']) {
    const first = make_symbol(description) as symbol
    const second = make_symbol(description) as symbol
    expect(typeof first).toBe('symbol')
    expect(first.description).toBe(description ?? undefined)
    expect(first).not.toBe(second)
    const object = {}
    expect(set(object, first, 42)).toBe(true)
    expect(get(object, first)).toBe(42)
    expect(has(object, second)).toBe(false)
  }
})

it('converts numeric keys using JavaScript rules', () => {
  const object = Object.create(null)
  for (const key of [0, -0, -1, 1.5, NaN, Infinity, -Infinity, 1e30]) {
    expect(set_number(object, key, key)).toBe(true)
    expect(get_number(object, key)).toBe(key)
    expect(object[String(key)]).toBe(key)
  }
  expect(get_index(['first', 'second'], 1)).toBe('second')
})

it('accepts symbols, raw JS strings, and coercible values as keys', () => {
  const object = Object.create(null)
  const symbol = Symbol('key')
  for (const key of [
    symbol,
    '\ud800',
    '\0',
    '\ufeffkey🦀',
    42n,
    true,
    null,
    undefined,
  ]) {
    const value = {}
    expect(set(object, key, value)).toBe(true)
    expect(get(object, key)).toBe(value)
    expect(has(object, key)).toBe(true)
    expect(deleteProperty(object, key)).toBe(true)
    expect(has(object, key)).toBe(false)
  }
  let conversions = 0
  const key = {
    [Symbol.toPrimitive](hint: string) {
      expect(hint).toBe('string')
      conversions++
      grow_memory()
      return symbol
    },
  }
  expect(set(object, key, 42)).toBe(true)
  expect(conversions).toBe(1)
  expect(get(object, key)).toBe(42)
  expect(conversions).toBe(2)
})

it('checks inherited properties but only deletes own properties', () => {
  const prototype = { inherited: 1 }
  const object = Object.assign(Object.create(prototype), { own: 2 })
  expect(has(object, 'inherited')).toBe(true)
  expect(descriptor(object, 'inherited')).toBeUndefined()
  expect(deleteProperty(object, 'inherited')).toBe(true)
  expect(has(object, 'inherited')).toBe(true)
  expect(deleteProperty(object, 'own')).toBe(true)
  expect(has(object, 'own')).toBe(false)
  expect(deleteProperty(object, 'missing')).toBe(true)
  expect(deleteProperty(Object.freeze({ value: 1 }), 'value')).toBe(false)
})

it('defines and reads data and accessor descriptors', () => {
  const object = {}
  const symbol = Symbol('property')
  const value = {}
  expect(
    define(object, symbol, {
      value,
      writable: true,
      enumerable: false,
      configurable: true,
    }),
  ).toBe(true)
  expect(descriptor(object, symbol)).toEqual({
    value,
    writable: true,
    enumerable: false,
    configurable: true,
  })
  const getter = function (this: unknown) {
    return this
  }
  expect(define(object, 'owner', { get: getter })).toBe(true)
  expect(get(object, 'owner')).toBe(object)
  expect((descriptor(object, 'owner') as PropertyDescriptor).get).toBe(getter)
  expect(define(Object.preventExtensions({}), 'new', { value })).toBe(false)
  expect(define(Object.freeze({ value: 1 }), 'value', { value: 2 })).toBe(false)
  expect(() => define(object, 'invalid', { value, get: getter })).toThrow(
    TypeError,
  )
  expect(() => define(object, 'invalid', 42)).toThrow(TypeError)
})

it('enumerates all own keys in JavaScript order and preserves exact keys', () => {
  const symbol = Symbol('property')
  const object = Object.create({ inherited: 1 })
  Object.defineProperties(object, {
    '10': { value: 10 },
    '2': { value: 2 },
    hidden: { value: 3 },
    '\ud800': { value: 4 },
    [symbol]: { value: 5 },
  })
  expect(own_keys(object)).toEqual(Reflect.ownKeys(object))
  const copy = copy_properties(object)
  expect(Reflect.ownKeys(copy as object)).toEqual(Reflect.ownKeys(object))
  expect(Object.getOwnPropertyDescriptors(copy)).toEqual(
    Object.getOwnPropertyDescriptors(object),
  )
  expect(own_keys({})).toEqual([])
})

it('calls symbol and numeric methods with the original receiver', () => {
  const key = Symbol('run')
  const object = {
    [key](this: unknown, arg: unknown) {
      return [this, arg]
    },
    12(this: unknown, arg: unknown) {
      return [this, arg]
    },
  }
  const argument = {}
  for (const method of [key, 12]) {
    const result = call(object, method, argument) as unknown[]
    expect(result[0]).toBe(object)
    expect(result[1]).toBe(argument)
  }
})

it('constructs classes, bound constructors, and constructor proxies', () => {
  class Example {
    constructor(
      public first?: unknown,
      public second?: unknown,
    ) {}
  }
  const first = {}
  const second = Symbol('argument')
  for (const constructor of [
    Example,
    Example.bind(null),
    new Proxy(Example, {}),
  ]) {
    const result = construct(constructor, [first, second]) as Example
    expect(result).toBeInstanceOf(Example)
    expect(result.first).toBe(first)
    expect(result.second).toBe(second)
  }
  expect(construct(Example, [])).toBeInstanceOf(Example)
  const replacement = {}
  expect(
    construct(function () {
      return replacement
    }, []),
  ).toBe(replacement)
  for (const invalid of [undefined, null, 42, {}, () => 42]) {
    expect(() => construct(invalid, [])).toThrow(TypeError)
  }
})

const exceptions = [
  undefined,
  null,
  false,
  0,
  NaN,
  'error',
  Symbol('error'),
  {},
  new Error('error'),
]

it.each(exceptions)('preserves the original exception %s', (exception) => {
  const key = {
    [Symbol.toPrimitive]() {
      throw exception
    },
  }
  const descriptorValue = {
    get value() {
      throw exception
    },
  }
  const proxy = new Proxy(
    {},
    {
      has() {
        throw exception
      },
      deleteProperty() {
        throw exception
      },
      ownKeys() {
        throw exception
      },
      getOwnPropertyDescriptor() {
        throw exception
      },
      defineProperty() {
        throw exception
      },
    },
  )
  const operations = [
    () => get({}, key),
    () => set({}, key, 42),
    () => has({}, key),
    () => deleteProperty({}, key),
    () => descriptor({}, key),
    () => define({}, key, {}),
    () => call({}, key, 42),
    () => has(proxy, 'key'),
    () => deleteProperty(proxy, 'key'),
    () => own_keys(proxy),
    () => descriptor(proxy, 'key'),
    () => define(proxy, 'key', {}),
    () => define({}, 'key', descriptorValue),
    () =>
      construct(function () {
        throw exception
      }, []),
  ]
  for (const operation of operations) {
    let caught = false
    try {
      operation()
    } catch (error) {
      caught = true
      expect(error).toBe(exception)
    }
    expect(caught).toBe(true)
  }
  expect(has({}, 'key')).toBe(false)
})

it('rejects primitive reflection targets', () => {
  for (const target of [undefined, null, 42, 'text', false, 42n, Symbol()]) {
    expect(() => has(target, 'key')).toThrow(TypeError)
    expect(() => deleteProperty(target, 'key')).toThrow(TypeError)
    expect(() => own_keys(target)).toThrow(TypeError)
    expect(() => descriptor(target, 'key')).toThrow(TypeError)
    expect(() => define(target, 'key', {})).toThrow(TypeError)
  }
})

it('supports re-entry and memory growth in reflection traps and constructors', () => {
  const target = { value: 42 }
  const proxy = new Proxy(target, {
    has(target, key) {
      grow_memory()
      return has(target, key)
    },
    ownKeys(target) {
      grow_memory()
      return own_keys(target) as (string | symbol)[]
    },
    getOwnPropertyDescriptor(target, key) {
      grow_memory()
      return descriptor(target, key) as PropertyDescriptor
    },
    defineProperty(target, key, value) {
      grow_memory()
      return define(target, key, value)
    },
    deleteProperty(target, key) {
      grow_memory()
      return deleteProperty(target, key)
    },
  })
  expect(has(proxy, 'value')).toBe(true)
  expect(own_keys(proxy)).toEqual(['value'])
  expect(descriptor(proxy, 'value')).toEqual(
    Object.getOwnPropertyDescriptor(target, 'value'),
  )
  expect(define(proxy, 'value', { value: 43 })).toBe(true)
  expect(deleteProperty(proxy, 'value')).toBe(true)
  const argument = {}
  const constructor = new Proxy(function () {}, {
    construct(_target, args) {
      grow_memory()
      return { value: get({ value: args[0] }, 'value') }
    },
  })
  expect(construct(constructor, [argument])).toEqual({ value: argument })
})
