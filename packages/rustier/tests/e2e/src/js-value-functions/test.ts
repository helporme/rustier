import { expect, it } from 'vitest'
import { ValueStore } from '../fn-js-value/test.rs'
import { grow_memory } from '../js-value-properties/test.rs'
import {
  apply0,
  apply2,
  apply_as0,
  apply_as2,
  apply_failed,
  apply_many,
  call0,
  call2,
  call_failed,
  caught_apply,
  caught_call,
} from './test.rs'

const values: [string, unknown][] = [
  ['undefined', undefined],
  ['null', null],
  ['false', false],
  ['zero', 0],
  ['negative zero', -0],
  ['NaN', NaN],
  ['string', 'returned'],
  ['bigint', 42n],
  ['symbol', Symbol('returned')],
  ['object', {}],
  ['array', [1, 2]],
  ['function', () => 42],
  ['Error', new Error('returned')],
]

it.each(values)('preserves a returned %s', (_name, value) => {
  const functionValue = () => value

  expect(apply_failed(functionValue)).toBe(false)
  expect(apply0(functionValue)).toBe(value)
})

it('passes arguments without copying their values', () => {
  const first = { name: 'first' }
  const second = Symbol('second')
  const result = apply2(
    (...args: unknown[]) => args,
    first,
    second,
  ) as unknown[]

  expect(result).toHaveLength(2)
  expect(result[0]).toBe(first)
  expect(result[1]).toBe(second)
})

it('uses undefined as the receiver for a strict function', () => {
  const functionValue = function (this: unknown) {
    return this
  }

  expect(apply0(functionValue)).toBeUndefined()
})

it('uses normal JavaScript receiver binding for a non-strict function', () => {
  const functionValue = Function('return this')

  expect(apply0(functionValue)).toBe(
    Reflect.apply(functionValue, undefined, []),
  )
  expect(apply_as0(functionValue, null)).toBe(
    Reflect.apply(functionValue, null, []),
  )
})

it.each(values)(
  'preserves an explicit %s receiver for a strict function',
  (_name, value) => {
    const functionValue = function (this: unknown) {
      return this
    }

    expect(apply_as0(functionValue, value)).toBe(value)
  },
)

it('passes both an explicit receiver and arguments', () => {
  const receiver = { name: 'receiver' }
  const first = {}
  const second = Symbol('argument')
  const functionValue = function (this: unknown, ...args: unknown[]) {
    return { receiver: this, args }
  }
  const result = apply_as2(functionValue, receiver, first, second) as {
    receiver: unknown
    args: unknown[]
  }

  expect(result.receiver).toBe(receiver)
  expect(result.args).toHaveLength(2)
  expect(result.args[0]).toBe(first)
  expect(result.args[1]).toBe(second)
})

it('supports bound functions and ignores unrelated apply/call properties', () => {
  const receiver = { name: 'bound' }
  const functionValue = function (this: unknown, ...args: unknown[]) {
    return { receiver: this, args }
  }.bind(receiver, 'bound argument')
  Object.defineProperties(functionValue, {
    apply: {
      get() {
        throw new Error('unexpected apply property access')
      },
    },
    call: {
      get() {
        throw new Error('unexpected call property access')
      },
    },
  })
  const result = apply_as2(functionValue, {}, 1, 2) as {
    receiver: unknown
    args: unknown[]
  }

  expect(result.receiver).toBe(receiver)
  expect(result.args).toEqual(['bound argument', 1, 2])
})

it('supports callable proxies', () => {
  const receiver = { name: 'receiver' }
  const argument = { name: 'argument' }
  let seenReceiver: unknown
  let seenArgs: unknown[] = []
  const proxy = new Proxy(() => undefined, {
    apply(_target, thisArg, args) {
      seenReceiver = thisArg
      seenArgs = args
      return args[0]
    },
  })

  expect(apply_as2(proxy, receiver, argument, 2)).toBe(argument)
  expect(seenReceiver).toBe(receiver)
  expect(seenArgs).toEqual([argument, 2])
})

it.each([0, 1, 12, 128])('passes %i arguments created in Rust', (count) => {
  const result = apply_many((...args: number[]) => args, count) as number[]

  expect(result).toEqual(Array.from({ length: count }, (_, index) => index))
})

it('calls an inherited method with the original receiver', () => {
  const prototype = {
    run(this: unknown, ...args: unknown[]) {
      return { receiver: this, args }
    },
  }
  const object = Object.create(prototype)
  const first = {}
  const second = Symbol('argument')
  const result = call2(object, 'run', first, second) as {
    receiver: unknown
    args: unknown[]
  }

  expect(result.receiver).toBe(object)
  expect(result.args[0]).toBe(first)
  expect(result.args[1]).toBe(second)
})

it('looks up a method once before invoking it', () => {
  let lookups = 0
  const object = {
    get run() {
      lookups++
      return function (this: unknown) {
        return this
      }
    },
  }

  expect(call0(object, 'run')).toBe(object)
  expect(lookups).toBe(1)
})

it('preserves Unicode, BOM, and embedded-NUL method names', () => {
  const object = Object.create(null)
  for (const key of ['', '\0', 'a\0b', '\ufeffmethod', '方法🦀']) {
    object[key] = function (this: unknown) {
      return this
    }
    expect(call0(object, key)).toBe(object)
  }
})

it.each(values)('returns the original thrown %s to Rust', (_name, value) => {
  const functionValue = () => {
    throw value
  }

  expect(apply_failed(functionValue)).toBe(true)
  expect(caught_apply(functionValue)).toBe(value)
  let caught = false
  try {
    apply0(functionValue)
  } catch (error) {
    caught = true
    expect(error).toBe(value)
  }
  expect(caught).toBe(true)
  expect(apply0(() => 42)).toBe(42)
})

it.each(values)(
  'preserves a thrown %s during method lookup and invocation',
  (_name, value) => {
    const getter = {
      get run() {
        throw value
      },
    }
    const method = {
      run() {
        throw value
      },
    }

    for (const object of [getter, method]) {
      expect(call_failed(object, 'run')).toBe(true)
      expect(caught_call(object, 'run')).toBe(value)
      let caught = false
      try {
        call0(object, 'run')
      } catch (error) {
        caught = true
        expect(error).toBe(value)
      }
      expect(caught).toBe(true)
    }
  },
)

it('does not inspect a thrown object', () => {
  const exception = new Proxy(
    {},
    {
      get() {
        throw new Error('unexpected exception inspection')
      },
    },
  )
  const functionValue = () => {
    throw exception
  }

  expect(caught_apply(functionValue)).toBe(exception)
  expect(caught_call({ run: functionValue }, 'run')).toBe(exception)
})

it('reports TypeError for non-callable values and methods', () => {
  for (const value of [
    undefined,
    null,
    false,
    42,
    'text',
    {},
    class Example {},
  ]) {
    expect(apply_failed(value)).toBe(true)
    expect((caught_apply(value) as Error).name).toBe('TypeError')
    expect(call_failed({ run: value }, 'run')).toBe(true)
    expect((caught_call({ run: value }, 'run') as Error).name).toBe('TypeError')
  }
  expect(call_failed({}, 'missing')).toBe(true)
  expect(call_failed('text', 'toString')).toBe(true)
})

it('captures errors from revoked callable proxies', () => {
  const { proxy, revoke } = Proxy.revocable(() => 42, {})
  revoke()

  expect(apply_failed(proxy)).toBe(true)
  expect((caught_apply(proxy) as Error).name).toBe('TypeError')
})

it('allows calls to re-enter Rust and grow Wasm memory', () => {
  const argument = { name: 'argument' }
  const functionValue = (first: unknown) => {
    grow_memory()
    return apply0(() => first)
  }
  expect(apply2(functionValue, argument, null)).toBe(argument)

  const object = {
    get run() {
      grow_memory()
      return functionValue
    },
  }
  expect(call2(object, 'run', argument, null)).toBe(argument)
})

it('can invoke a function retained in Rust across calls', () => {
  const functionValue = () => 42
  const store = new ValueStore(functionValue)

  expect(apply0(store.get())).toBe(42)
  store.clear()
})
