import { expect, it } from 'vitest'
import {
  make_struct_variant,
  make_tuple_variant,
  make_unit_variant,
} from './test.rs'

const crab = '🦀'.codePointAt(0)!
const rocket = '🚀'.codePointAt(0)!

it('exhaustively exposes every MixedEnum variant and supported field type', () => {
  const values = [
    make_unit_variant(),
    make_tuple_variant(),
    make_struct_variant(),
  ]

  for (const value of values) {
    switch (value.type) {
      case 'Unit':
        expect({ type: value.type }).toEqual({ type: 'Unit' })
        break

      case 'Tuple': {
        if (value[12].type !== 'Value')
          throw new Error('expected NestedEnum::Value')

        expect({
          type: value.type,
          flag: value[0],
          character: value[1],
          text: value[2],
          i8_value: value[3],
          i16_value: value[4],
          i32_value: value[5],
          u8_value: value[6],
          u16_value: value[7],
          u32_value: value[8],
          f32_value: value[9],
          f64_value: value[10],
          struct_value: value[11].value,
          enum_type: value[12].type,
          enum_value: value[12].value,
        }).toEqual({
          type: 'Tuple',
          flag: true,
          character: crab,
          text: 'tuple',
          i8_value: -8,
          i16_value: -16,
          i32_value: -32,
          u8_value: 8,
          u16_value: 16,
          u32_value: 32,
          f32_value: 32.5,
          f64_value: 64.5,
          struct_value: 100,
          enum_type: 'Value',
          enum_value: 200,
        })
        break
      }

      case 'Struct': {
        if (value.enum_value.type !== 'Value')
          throw new Error('expected NestedEnum::Value')

        expect({
          type: value.type,
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
          type: 'Struct',
          flag: false,
          character: rocket,
          text: 'struct',
          i8_value: -18,
          i16_value: -116,
          i32_value: -132,
          u8_value: 18,
          u16_value: 116,
          u32_value: 132,
          f32_value: 42.5,
          f64_value: 84.5,
          struct_value: 300,
          enum_type: 'Value',
          enum_value: 400,
        })
        break
      }

      default: {
        const exhaustive: never = value
        throw new Error(`unexpected MixedEnum variant: ${String(exhaustive)}`)
      }
    }
  }
})
