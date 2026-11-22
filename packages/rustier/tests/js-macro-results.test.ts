import { expect, test } from 'vitest'
import { rs } from 'rustier'

const placements = [
  {
    name: 'inline rs',
    noInputs: () => rs`js!(return "no inputs")`,
    effect: (callback: (arg: string) => number) => rs`
      let callback: JsValue = ${callback};
      let result: Result<(), JsCallError> = js!(@{callback}("called"));
      result
    `,
    fail: () => rs`js!(throw new Error("inline rs exception"))`,
  },
  {
    name: 'handy-crate',
    noInputs: () => rs`handy_crate::js_no_inputs().at(__rustier_env)`,
    effect: (callback: (arg: string) => number) => rs`handy_crate::js_effect(${callback}).at(__rustier_env)`,
    fail: () => rs`handy_crate::js_throw().at(__rustier_env)`,
  },
]

for (const placement of placements) {
  test(`${placement.name}: returns a value without any inputs`, () => {
    expect(placement.noInputs()).toBe('no inputs')
  })

  test(`${placement.name}: a side-effect-only call returns unit`, () => {
    const calls: string[] = []
    expect(placement.effect(arg => calls.push(arg))).toBeUndefined()
    expect(calls).toEqual(['called'])
  })

  test(`${placement.name}: propagates JavaScript exceptions`, () => {
    expect(placement.fail).toThrow(`${placement.name} exception`)
  })
}

test('runs the selected snippet and moves owned Rust inputs', () => {
  const run = (enabled: boolean) => rs`
    let enabled: bool = ${enabled};
    if enabled {
      js!(return @{"Hello world".to_string()})
    } else {
      js!(return undefined)
    }
  `
  expect(run(true)).toBe('Hello world')
  expect(run(false)).toBeUndefined()
})

test('surfaces thrown JavaScript errors', () => {
  expect(() => rs`js!(throw new Error("boom"))`).toThrow('boom')
})

test('preserves line-sensitive JavaScript and adjacent operator tokens', () => {
  expect(rs`js!(return
    3
  )`).toBeUndefined()
  expect(rs`js!(return @{3u32} === 3 && (null ?? 4) === 4)`).toBe(true)
})
