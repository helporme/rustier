import { expect, it } from 'vitest'
import { accepts_env, add_with_env, same_env } from './test.rs'

it('supplies JsEnv to Rust without a JavaScript argument', () => {
  expect(accepts_env.length).toBe(0)
  expect(accepts_env()).toBe(true)
})

it('preserves ordinary arguments around an injected JsEnv', () => {
  expect(add_with_env.length).toBe(2)
  expect(add_with_env(12, 30)).toBe(42)
})

it('supplies the same environment to multiple JsEnv parameters', () => {
  expect(same_env.length).toBe(0)
  expect(same_env()).toBe(true)
})
