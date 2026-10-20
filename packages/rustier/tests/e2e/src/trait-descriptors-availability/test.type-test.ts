import { expectTypeOf, test } from 'vitest'
import * as contracts from './contracts.rs'

test('omits traits without a discovered supported surface', () => {
  expectTypeOf(contracts).not.toHaveProperty('Marker')
  expectTypeOf(contracts).not.toHaveProperty('Unsupported')
  expectTypeOf(contracts).not.toHaveProperty('Unused')
})
