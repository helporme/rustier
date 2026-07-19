import { expect, it } from 'vitest'
import {
  make_mixed_struct,
  make_nested_enum,
  make_nested_struct,
} from './test.rs'

const crab = '🦀'.codePointAt(0)!
const rocket = '🚀'.codePointAt(0)!

it('reads and writes every supported MixedStruct field type', () => {
  const value = make_mixed_struct()
  if (value.enum_value.type !== 'value')
    throw new Error('expected NestedEnum::Value')

  expect({
    flag: value.flag,
    character: value.character,
    text: value.text,
    i8_value: value.i8_value,
    i16_value: value.i16_value,
    i32_value: value.i32_value,
    u8_value: value.u8_value,
    u16_value: value.u16_value,
    u32_value: value.u32_value,
    f32_value: value.f32_value,
    f64_value: value.f64_value,
    struct_value: value.struct_value.value,
    enum_type: value.enum_value.type,
    enum_value: value.enum_value.value,
  }).toEqual({
    flag: true,
    character: crab,
    text: 'owned',
    i8_value: -8,
    i16_value: -16,
    i32_value: -32,
    u8_value: 8,
    u16_value: 16,
    u32_value: 32,
    f32_value: 32.5,
    f64_value: 64.5,
    struct_value: 100,
    enum_type: 'value',
    enum_value: 200,
  })

  value.flag = false
  value.character = rocket
  value.text = 'updated'
  value.i8_value = -18
  value.i16_value = -116
  value.i32_value = -132
  value.u8_value = 18
  value.u16_value = 116
  value.u32_value = 132
  value.f32_value = 42.5
  value.f64_value = 84.5
  value.struct_value = make_nested_struct(300)
  value.enum_value = make_nested_enum(400)

  if (value.enum_value.type !== 'value')
    throw new Error('expected NestedEnum::Value')

  expect({
    flag: value.flag,
    character: value.character,
    text: value.text,
    i8_value: value.i8_value,
    i16_value: value.i16_value,
    i32_value: value.i32_value,
    u8_value: value.u8_value,
    u16_value: value.u16_value,
    u32_value: value.u32_value,
    f32_value: value.f32_value,
    f64_value: value.f64_value,
    struct_value: value.struct_value.value,
    enum_type: value.enum_value.type,
    enum_value: value.enum_value.value,
  }).toEqual({
    flag: false,
    character: rocket,
    text: 'updated',
    i8_value: -18,
    i16_value: -116,
    i32_value: -132,
    u8_value: 18,
    u16_value: 116,
    u32_value: 132,
    f32_value: 42.5,
    f64_value: 84.5,
    struct_value: 300,
    enum_type: 'value',
    enum_value: 400,
  })
})
