import { expect, it } from 'vitest'
import { Resource, Shipment, consume, drops } from './test.rs'

it('drops an owned argument clone when converting a later numeric argument fails', () => {
  const resource = new Resource(42)
  const before = drops()

  expect(() => consume(resource, 128)).toThrow('cannot cast 128 to i8')
  expect(drops() - before).toBe(1)
  expect(resource.value()).toBe(42)

  expect(consume(resource, 1)).toBe(43)
  expect(drops() - before).toBe(2)
  expect(resource.value()).toBe(42)
})

it('drops a cloned field when constructing an enclosing object fails', () => {
  const resource = new Resource(42)
  const before = drops()

  expect(() => Shipment.of({ resource, count: 128 })).toThrow('cannot cast 128 to i8')
  expect(drops() - before).toBe(1)
  expect(resource.value()).toBe(42)

  const shipment = Shipment.of({ resource, count: 1 })

  expect(shipment.resource.value()).toBe(42)
  expect(shipment.count).toBe(1)
  expect(resource.value()).toBe(42)
})
