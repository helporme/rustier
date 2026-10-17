import { expect, it } from 'vitest'
import { Preferences } from '../option-coercion/test.rs'

it('rejects tagged Option objects in method arguments and fields', () => {
  const preferences = new Preferences(42)

  for (const input of [{ type: 'Some', 0: 7 }, { type: 'None' }]) {
    expect(() => preferences.replace_number(input as never)).toThrow()
    expect(() => { preferences.number = input as never }).toThrow()
    expect(preferences.number_or(-1)).toBe(42)
  }
})

it('rejects invalid payloads without replacing existing Option values', () => {
  const preferences = new Preferences(42)
  preferences.nested = 3

  expect(() => preferences.replace_number('7' as never)).toThrow()
  expect(() => { preferences.number = '7' as never }).toThrow()
  expect(() => preferences.replace_nested('3' as never)).toThrow()
  expect(() => { preferences.nested = '3' as never }).toThrow()
  expect(preferences.number_or(-1)).toBe(42)
  expect(preferences.nested_or(-1)).toBe(3)
})
