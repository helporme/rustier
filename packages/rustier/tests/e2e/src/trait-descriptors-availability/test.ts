import { expect, it } from 'vitest'
import * as contracts from './contracts.rs'
import { Widget } from './test.rs'

it('omits unsupported and undiscovered traits without adding selectors or shortcuts', () => {
  expect(contracts).not.toHaveProperty('Marker')
  expect(contracts).not.toHaveProperty('Unsupported')
  expect(contracts).not.toHaveProperty('Unused')
  expect(Widget).not.toHaveProperty('as')
  expect(Widget).not.toHaveProperty('from')
  const widget = Widget.of({ 0: 7 })
  expect(widget).not.toHaveProperty('as')
  expect(widget).not.toHaveProperty('clone')
})
