import { expectTypeOf, test } from 'vitest'
import { Thermometer } from './test.rs'

test('combines declarations from impl blocks in different modules', () => {
  const thermometer = Thermometer.new(18)

  expectTypeOf(thermometer).toEqualTypeOf<Thermometer>()
  expectTypeOf(thermometer.temperature()).toEqualTypeOf<number>()
  expectTypeOf(thermometer.adjust(1)).toEqualTypeOf<void>()
})
