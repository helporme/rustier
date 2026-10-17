import { expectTypeOf, test } from 'vitest'
import { Option } from '$rust/core/option'
import { Override, Preferences, Setting } from './test.rs'

test('accepts nullable payloads in method inputs and preserves Option return types', () => {
  expectTypeOf<Option.Coerce<'i32'>>().toEqualTypeOf<Option<'i32'> | number | null | undefined>()
  expectTypeOf<ConstructorParameters<typeof Preferences>>().toEqualTypeOf<[Option.Coerce<'i32'>]>()
  expectTypeOf<Parameters<typeof Preferences.from_number>>().toEqualTypeOf<[Option.Coerce<'i32'>]>()
  expectTypeOf<Parameters<Preferences['replace_number']>>().toEqualTypeOf<[Option.Coerce<'i32'>]>()
  expectTypeOf<Parameters<Preferences['replace_nested']>>().toEqualTypeOf<[Option.Coerce<Option.Class<'i32'>>]>()

  const preferences = new Preferences(null)
  expectTypeOf(preferences.replace_number(undefined)).toEqualTypeOf<Option<'i32'>>()
})

test('accepts nullable setter inputs while getters remain Option wrappers', () => {
  const preferences = Preferences.of({ number: undefined, enabled: false, label: '', nested: null })
  preferences.number = 0
  preferences.enabled = null
  preferences.label = undefined
  preferences.nested = 3

  expectTypeOf(preferences.number).toEqualTypeOf<Option<'i32'>>()
  expectTypeOf(preferences.enabled).toEqualTypeOf<Option<'bool'>>()
  expectTypeOf(preferences.label).toEqualTypeOf<Option<'String'>>()
  expectTypeOf(preferences.nested).toEqualTypeOf<Option<Option.Class<'i32'>>>()

  const override = Override.of({ 0: undefined })
  override[0] = null
  expectTypeOf(override[0]).toEqualTypeOf<Option<'i32'>>()

  const setting = Setting.of({ type: 'Named', value: 0 })
  if (setting.type === 'Named') {
    setting.value = undefined
    expectTypeOf(setting.value).toEqualTypeOf<Option<'i32'>>()
  }
})

test('rejects tagged Option inputs and invalid payloads at method and field boundaries', () => {
  const preferences = new Preferences(null)

  // @ts-expect-error Option inputs are payloads, not tagged objects.
  preferences.replace_number({ type: 'Some', 0: 7 })
  // @ts-expect-error Option fields no longer accept tagged None objects.
  preferences.number = { type: 'None' }
  // @ts-expect-error Option payloads retain their scalar type.
  preferences.replace_number('7')
  // @ts-expect-error Nested Option coercion retains its payload type.
  preferences.nested = '3'
})
