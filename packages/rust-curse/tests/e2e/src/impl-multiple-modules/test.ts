import { expect, it } from 'vitest'
import { Thermometer } from './test.rs'

it('combines impl blocks from different modules on one class', () => {
  const thermometer = new Thermometer(18)

  expect(thermometer.temperature()).toBe(18)

  thermometer.adjust(6)

  expect(thermometer.temperature()).toBe(24)
})

it('omits non-public methods declared in other modules', () => {
  const thermometer = new Thermometer(18) as unknown as Record<string, unknown>

  expect(thermometer.private_reading).toBeUndefined()
  expect(thermometer.module_reading).toBeUndefined()
  expect(thermometer.reset).toBeUndefined()
})
