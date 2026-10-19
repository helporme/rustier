import { expect, it } from 'vitest'
import {
  caught_get,
  caught_set,
  get,
  get_failed,
  grow_memory,
  make_record,
  retain_property,
  set,
  set_failed,
} from './test.rs'

it('reads own and inherited properties with the original receiver', () => {
  const marker = { name: 'receiver' }
  const prototype = {
    inherited: 42,
    get owner(): unknown {
      return Reflect.get(this, 'marker')
    },
  }
  const object = Object.assign(Object.create(prototype), { marker })

  expect(get(object, 'inherited')).toBe(42)
  expect(get(object, 'marker')).toBe(marker)
  expect(get(object, 'owner')).toBe(marker)
  expect(get(object, 'missing')).toBeUndefined()
  expect(get_failed(object, 'missing')).toBe(false)
  expect(get([10, 20], '1')).toBe(20)
  expect(get(() => 42, 'length')).toBe(0)
})

it('sets properties without copying or coercing their values', () => {
  const object: Record<string, unknown> = {}
  const values: unknown[] = [
    undefined,
    null,
    false,
    -0,
    NaN,
    '',
    42n,
    Symbol('value'),
    {},
    [1, 2],
    () => 42,
  ]

  for (const value of values) {
    expect(set(object, 'value', value)).toBe(true)
    expect(object.value).toBe(value)
    expect(get(object, 'value')).toBe(value)
  }
})

it('preserves empty, Unicode, BOM, and embedded-NUL property keys', () => {
  const object = Object.create(null)

  for (const key of ['', '\0', 'a\0b', '\ufeffkey', '你好🦀']) {
    const value = { key }
    expect(set(object, key, value)).toBe(true)
    expect(object[key]).toBe(value)
    expect(get(object, key)).toBe(value)
  }
})

it('returns false rather than an exception when Reflect.set rejects a write', () => {
  const readonly = Object.freeze({ value: 1 })
  const nonExtensible = Object.preventExtensions({})
  const getterOnly = {
    get value() {
      return 1
    },
  }

  for (const object of [readonly, nonExtensible, getterOnly]) {
    expect(set(object, 'value', 2)).toBe(false)
    expect(set_failed(object, 'value', 2)).toBe(false)
    expect(caught_set(object, 'value', 2)).toBe(false)
  }
  expect(readonly.value).toBe(1)
  expect(getterOnly.value).toBe(1)
})

it('calls inherited setters with the original receiver', () => {
  const prototype = {
    set value(value: unknown) {
      Reflect.set(this, 'saved', value)
    },
  }
  const object = Object.create(prototype)
  const value = () => 42

  expect(set(object, 'value', value)).toBe(true)
  expect(object.saved).toBe(value)
  expect(Object.hasOwn(prototype, 'saved')).toBe(false)
})

it('preserves proxy receivers for reads and writes', () => {
  let getReceiver: unknown
  let setReceiver: unknown
  const target = { value: 1 }
  const proxy = new Proxy(target, {
    get(target, key, receiver) {
      getReceiver = receiver
      return Reflect.get(target, key, receiver)
    },
    set(target, key, value, receiver) {
      setReceiver = receiver
      return Reflect.set(target, key, value, receiver)
    },
  })

  expect(get(proxy, 'value')).toBe(1)
  expect(getReceiver).toBe(proxy)
  expect(set(proxy, 'value', 2)).toBe(true)
  expect(setReceiver).toBe(proxy)
  expect(target.value).toBe(2)
})

it('reports a rejected proxy write as false', () => {
  const target = { value: 1 }
  const proxy = new Proxy(target, { set: () => false })

  expect(set(proxy, 'value', 2)).toBe(false)
  expect(set_failed(proxy, 'value', 2)).toBe(false)
  expect(target.value).toBe(1)
})

const exceptions: [string, unknown][] = [
  ['undefined', undefined],
  ['null', null],
  ['false', false],
  ['zero', 0],
  ['NaN', NaN],
  ['string', 'failure'],
  ['bigint', 42n],
  ['symbol', Symbol('failure')],
  ['object', { name: 'failure' }],
  ['function', () => 42],
  ['Error', new Error('failure')],
]

it.each(exceptions)(
  'returns the original thrown %s to Rust from a getter',
  (_name, exception) => {
    const object = {
      get value() {
        throw exception
      },
      good: 42,
    }

    expect(get_failed(object, 'value')).toBe(true)
    expect(caught_get(object, 'value')).toBe(exception)

    let caught = false
    try {
      get(object, 'value')
    } catch (error) {
      caught = true
      expect(error).toBe(exception)
    }
    expect(caught).toBe(true)
    expect(get(object, 'good')).toBe(42)
  },
)

it.each(exceptions)(
  'returns the original thrown %s to Rust from a setter',
  (_name, exception) => {
    let writes = 0
    const object = {
      set value(_value: unknown) {
        writes++
        throw exception
      },
      good: 42,
    }

    expect(set_failed(object, 'value', 'replacement')).toBe(true)
    expect(caught_set(object, 'value', 'replacement')).toBe(exception)

    let caught = false
    try {
      set(object, 'value', 'replacement')
    } catch (error) {
      caught = true
      expect(error).toBe(exception)
    }
    expect(caught).toBe(true)
    expect(writes).toBe(3)
    expect(get(object, 'good')).toBe(42)
  },
)

it('does not read or stringify a thrown object while capturing it', () => {
  const exception = new Proxy(
    {},
    {
      get() {
        throw new Error('unexpected exception inspection')
      },
    },
  )
  const object = {
    get value() {
      throw exception
    },
    set value(_value: unknown) {
      throw exception
    },
  }

  expect(caught_get(object, 'value')).toBe(exception)
  expect(get_failed(object, 'value')).toBe(true)
  expect(caught_set(object, 'value', 42)).toBe(exception)
  expect(set_failed(object, 'value', 42)).toBe(true)
})

it('rejects primitive targets instead of boxing them', () => {
  for (const value of [
    undefined,
    null,
    42,
    true,
    'text',
    42n,
    Symbol('target'),
  ]) {
    expect(get_failed(value, 'length')).toBe(true)
    expect(set_failed(value, 'value', 1)).toBe(true)
    expect((caught_get(value, 'length') as Error).name).toBe('TypeError')
    expect((caught_set(value, 'value', 1) as Error).name).toBe('TypeError')
  }
})

it('captures errors from revoked proxies', () => {
  const { proxy, revoke } = Proxy.revocable({}, {})
  revoke()

  expect(get_failed(proxy, 'value')).toBe(true)
  expect(set_failed(proxy, 'value', 1)).toBe(true)
  expect((caught_get(proxy, 'value') as Error).name).toBe('TypeError')
})

it('supports getter and setter re-entry into Rust, including memory growth', () => {
  const nested = { value: 1 }
  const object = {
    get value() {
      grow_memory()
      return get(nested, 'value')
    },
    set value(value: unknown) {
      grow_memory()
      set(nested, 'value', value)
    },
  }

  expect(get(object, 'value')).toBe(1)
  expect(set(object, 'value', 42)).toBe(true)
  expect(get(object, 'value')).toBe(42)
})

it('can retain a property result after the original property is removed', () => {
  const value = { name: 'retained' }
  const object: { value?: unknown } = { value }
  const store = retain_property(object, 'value')

  delete object.value
  expect(store.get()).toBe(value)
  store.clear()
})

it('can create an object and set its property entirely inside Rust', () => {
  const value = Symbol('payload')
  const object = make_record(value) as { value: unknown }

  expect(object.value).toBe(value)
  expect(get(object, 'value')).toBe(value)
})
