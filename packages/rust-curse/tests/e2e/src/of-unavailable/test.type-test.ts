import { expectTypeOf, test } from 'vitest'
import {
  ClosedEnum,
  NotClone,
  NotCloneEnum,
  OpenRecord,
  Owner,
  PrivateField,
  Shadowed,
  UnsupportedField,
} from './test.rs'

test('omits coerce methods for incomplete or non-Clone inputs', () => {
  expectTypeOf(NotClone).not.toHaveProperty('of')
  expectTypeOf(PrivateField).not.toHaveProperty('of')
  expectTypeOf(UnsupportedField).not.toHaveProperty('of')
  expectTypeOf(OpenRecord).not.toHaveProperty('of')
  expectTypeOf(Owner).not.toHaveProperty('of')
  expectTypeOf(NotCloneEnum).not.toHaveProperty('of')
  expectTypeOf(ClosedEnum).not.toHaveProperty('of')
  expectTypeOf(Shadowed.of({ value: 42 })).toEqualTypeOf<Shadowed>()
  expectTypeOf<Shadowed.Coerce>().toEqualTypeOf<{ value: number }>()
  expectTypeOf<Shadowed.Plain>().toEqualTypeOf<{ value: number }>()
  expectTypeOf(Shadowed.of(Shadowed.of({ value: 42 }))).toEqualTypeOf<Shadowed>()
})

test('omits Coerce along with unavailable coerce methods', () => {
  // @ts-expect-error The owner must implement Clone.
  type NoCloneInput = NotClone.Coerce
  // @ts-expect-error All source fields must be public.
  type PrivateInput = PrivateField.Coerce
  // @ts-expect-error All source fields must be supported.
  type UnsupportedInput = UnsupportedField.Coerce
  // @ts-expect-error Non-exhaustive structs cannot be constructed.
  type OpenInput = OpenRecord.Coerce
  // @ts-expect-error Nested class fields must be passable by Clone.
  type OwnerInput = Owner.Coerce
  // @ts-expect-error Parent enums must implement Clone.
  type EnumInput = NotCloneEnum.Coerce
  // @ts-expect-error No variants can be constructed.
  type ClosedInput = ClosedEnum.Coerce
})
