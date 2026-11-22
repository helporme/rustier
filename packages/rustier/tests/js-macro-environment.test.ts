import { expect, test } from 'vitest'
import { rs } from 'rustier'

test('calls JavaScript from a Rust dependency using an explicit environment', () => {
  const x = 3
  expect(rs`handy_crate::point(__rustier_env, ${x}, 7)`).toEqual({ x: 3, y: 7 })
})

test('an opaque deferred call can use the environment of a JS value', () => {
  const parent = { child: {} }
  expect(rs`
    let parent: JsValue = ${parent};
    let child = js!(return @{parent}.child).unwrap();
    let call = handy_crate::deferred(32u32);
    let env: JsEnv = child.env();
    call.at(env)
  `).toBe(32)
})
