import { expect, it } from 'vitest'
import { Header, reserved_parameters } from './test.rs'

it('exposes raw identifiers and JS-sensitive property names without changing the prototype', () => {
  const header = Header.of({ type: 'text/plain', constructor: 42, ['__proto__']: 7 })

  expect(header.type).toBe('text/plain')
  expect(header.constructor).toBe(42)
  expect(header.__proto__).toBe(7)
  expect(Object.getPrototypeOf(header)).toBe(Header.prototype)
  header.__proto__ = 8
  header.delete()

  expect(header.__proto__).toBe(8)
  expect(header.constructor).toBe(0)
  expect(Header.default().type).toBe('default')
})

it('accepts Rust parameters whose names are reserved in JavaScript', () => {
  expect(reserved_parameters(10, 20, 12)).toBe(42)
})
