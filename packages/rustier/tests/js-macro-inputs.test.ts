import { expect, test } from 'vitest'
import { rs } from 'rustier'

const placements = [
  {
    name: 'inline rs',
    inputAt: (object: object, index: number) => rs`
      let object: JsValue = ${object};
      let index: u8 = ${index};
      js!(return [
        @{i8::MIN}, @{i16::MIN}, @{i32::MIN}, @{u8::MAX}, @{u16::MAX}, @{u32::MAX},
        @{1.25f32}, @{-2.5f64}, @{String::from("héllo")}, @{true}, @{()}, @{object}, @{handy_crate::CopyValue(73)}
      ][@{index}])
    `,
  },
  {
    name: 'handy-crate',
    inputAt: (object: object, index: number) => rs`
      let object: JsValue = ${object};
      handy_crate::js_input_at(object.env(), object, ${index})
    `,
  },
]

for (const placement of placements) {
  test(`${placement.name}: returns primitive inputs, JS values and Rust handles`, () => {
    const object = {}
    const primitives = [
      -128, -32768, -2147483648, 255, 65535, 4294967295,
      1.25, -2.5, 'héllo', true, undefined,
    ]
    primitives.forEach((expected, index) => expect(placement.inputAt(object, index)).toBe(expected))
    expect(placement.inputAt(object, 11)).toBe(object)
    const handle = placement.inputAt(object, 12)
    expect(rs`let item: handy_crate::CopyValue = ${handle}; item.0`).toBe(73)
  })
}

test('preserves JS identity and wraps an opaque Rust input as an Rs handle', () => {
  const object = {}
  const pair = rs`
    let object: JsValue = ${object};
    js!(
      const item = @{handy_crate::CopyValue(13)};
      return { object: @{object}, item };
    )
  `
  expect(pair.object).toBe(object)
  expect(rs`let item: handy_crate::CopyValue = ${pair.item}; item.0`).toBe(13)
})

test('evaluates interpolations once in order and supports reentrant calls', () => {
  const callback = () => rs`js!(return 3)`
  expect(rs`
    let callback: rustier::JsValue = ${callback};
    let mut n = 0;
    js!(return [@{{ n += 1; n }}, @{callback}(), @{{ n += 1; n }}])
  `).toEqual([1, 3, 2])
})

