import { expect, test } from 'vitest'
import { rs } from 'rustier'

test('calls JavaScript from a Rust dependency using an explicit environment', () => {
  const x = 3
  expect(rs`handy_crate::point(__rustier_env, ${x}, 7)`).toEqual({ x: 3, y: 7 })
})

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

test('surfaces thrown JavaScript errors', () => {
  expect(() => rs`js!(throw new Error("boom"))`).toThrow('boom')
})

test('an opaque deferred call can use the environment of a JS value', () => {
  const parent = { child: {} }
  expect(rs`
    let parent: JsValue = ${parent};
    let child = parent.get("child").unwrap();
    let call = handy_crate::deferred(32u32);
    let env: JsEnv = child.env();
    call.at(env)
  `).toBe(32)
})

test('evaluates interpolations once in order and supports reentrant calls', () => {
  const callback = () => rs`js!(return 3)`
  expect(rs`
    let callback: rustier::JsValue = ${callback};
    let mut n = 0;
    js!(return [@{{ n += 1; n }}, @{callback}(), @{{ n += 1; n }}])
  `).toEqual([1, 3, 2])
})

test('preserves line-sensitive JavaScript and adjacent operator tokens', () => {
  expect(rs`js!(return
    3
  )`).toBeUndefined()
  expect(rs`js!(return @{3u32} === 3 && (null ?? 4) === 4)`).toBe(true)
})
