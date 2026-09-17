import { expectTypeOf, test } from 'vitest'
import {
  ChildStruct,
  ParentStruct,
  NonCloneParentStruct,
  child_value,
  make_child_struct,
  make_non_clone_parent_struct,
} from './test.rs'

test('declares public struct fields as a plain object type', () => {
  const plain: ChildStruct.Plain = { value: 42 }
  const instance = make_child_struct(42)

  expectTypeOf<ChildStruct.Plain>().toEqualTypeOf<{ value: number }>()
  expectTypeOf(instance).toExtend<ChildStruct.Plain>()
  expectTypeOf(plain.value).toEqualTypeOf<number>()
  plain.value = 43

  // @ts-expect-error Plain fields retain their requiredness.
  const missing: ChildStruct.Plain = {}
  // @ts-expect-error Plain is a type, not a runtime property.
  ChildStruct.Plain
})

test('accepts plain function inputs without making plain values branded instances', () => {
  const plain: ChildStruct.Plain = { value: 42 }

  // @ts-expect-error Plain objects do not carry the Rust instance brand.
  const instance: ChildStruct = plain
  expectTypeOf(child_value(plain)).toEqualTypeOf<number>()
})

test('keeps nested fields as Rust instance types', () => {
  const plain: ParentStruct.Plain = { child: make_child_struct(42) }

  expectTypeOf(plain.child).toEqualTypeOf<ChildStruct>()
  // @ts-expect-error Plain does not recursively coerce nested object fields.
  const nested: ParentStruct.Plain = { child: { value: 42 } }
})

test('preserves readonly fields in plain types', () => {
  const parent = make_non_clone_parent_struct(42)
  const plain: NonCloneParentStruct.Plain = { child: parent.child }

  expectTypeOf(plain.child).toEqualTypeOf<typeof parent.child>()
  // @ts-expect-error The non-Clone child field remains readonly.
  plain.child = parent.child
})
