import { expect, it } from 'vitest'
import { Option } from '$rust/core/option'
import { Override, Preferences, Setting } from './test.rs'

it('coerces numbers and nullish values in constructors and static methods', () => {
  for (const input of [0, 7, null, undefined]) {
    const expected = input ?? -1

    expect(new Preferences(input).number_or(-1)).toBe(expected)
    expect(Preferences.from_number(input).number_or(-1)).toBe(expected)
  }
})

it('coerces Option arguments in mutating instance methods and keeps returned wrappers', () => {
  const preferences = new Preferences(null)

  for (const input of [0, null, 7, undefined]) {
    const previous = preferences.number_or(-1)
    const returned = preferences.replace_number(input)

    expect(returned).toBeInstanceOf(Option('i32'))
    expect(returned.unwrap_or(-1)).toBe(previous)
    expect(preferences.number_or(-1)).toBe(input ?? -1)
  }
})

it('accepts existing Option instances in methods without changing their values', () => {
  const OptionalNumber = Option('i32')
  const some = OptionalNumber.of(42)
  const none = OptionalNumber.of(null)
  const preferences = new Preferences(some)

  expect(OptionalNumber.of(some)).toBe(some)
  expect(preferences.number_or(-1)).toBe(42)
  preferences.replace_number(none)
  expect(preferences.number_or(-1)).toBe(-1)
  preferences.replace_number(some)
  expect(preferences.number_or(-1)).toBe(42)
  expect(some.unwrap()).toBe(42)
  expect(none.is_none()).toBe(true)
})

it('coerces Option fields during object construction and named field assignment', () => {
  const preferences = Preferences.of({
    number: 0,
    enabled: false,
    label: '',
    nested: null,
  })

  expect(preferences.number_or(-1)).toBe(0)
  expect(preferences.enabled.unwrap()).toBe(false)
  expect(preferences.label.unwrap()).toBe('')
  expect(preferences.nested.is_none()).toBe(true)

  for (const input of [7, null, 0, undefined]) {
    preferences.number = input
    expect(preferences.number).toBeInstanceOf(Option('i32'))
    expect(preferences.number_or(-1)).toBe(input ?? -1)
  }

  preferences.enabled = undefined
  preferences.label = null
  expect(preferences.enabled.is_none()).toBe(true)
  expect(preferences.label.is_none()).toBe(true)
  preferences.enabled = false
  preferences.label = ''
  expect(preferences.enabled.unwrap()).toBe(false)
  expect(preferences.label.unwrap()).toBe('')

  const some = Option('i32').of(42)
  preferences.number = some
  expect(preferences.number_or(-1)).toBe(42)
  expect(some.unwrap()).toBe(42)
})

it('recursively coerces nested Option payloads in method arguments and fields', () => {
  const preferences = new Preferences(null)

  preferences.replace_nested(3)
  expect(preferences.nested_or(-1)).toBe(3)
  expect(preferences.nested.unwrap()).toBeInstanceOf(Option('i32'))
  preferences.nested = 5
  expect(preferences.nested_or(-1)).toBe(5)
  preferences.nested = Option('i32').of(null)
  expect(preferences.nested.is_some()).toBe(true)
  expect(preferences.nested.unwrap().is_none()).toBe(true)
  preferences.replace_nested(undefined)
  expect(preferences.nested_or(-1)).toBe(-1)
  preferences.nested = null
  expect(preferences.nested.is_none()).toBe(true)
})

it('coerces Option fields in tuple structs and named and tuple enum variants', () => {
  const override = Override.of({ 0: null })
  const named = Setting.of({ type: 'Named', value: undefined })
  const tuple = Setting.of({ type: 'Tuple', 0: null })

  if (named.type !== 'Named' || tuple.type !== 'Tuple') {
    throw new Error('expected Setting::Named and Setting::Tuple')
  }

  for (const input of [0, null, 7, undefined]) {
    override[0] = input
    named.value = input
    tuple[0] = input

    expect(override[0].unwrap_or(-1)).toBe(input ?? -1)
    expect(named.value.unwrap_or(-1)).toBe(input ?? -1)
    expect(tuple[0].unwrap_or(-1)).toBe(input ?? -1)
  }
})
