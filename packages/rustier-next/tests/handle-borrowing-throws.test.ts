import { expect, test } from 'vitest'
import { rs } from 'rustier-next'
import { rustierDiags } from '$build/diags'

const diagnostics = rustierDiags.filter(message => message.includes('/handle-borrowing-throws.test.ts:'))

test('shared references report diagnostics and throw', () => {
  const invalid = () => rs`let value: &u32 = &42; value`
  const error = diagnostics.find(message => message.includes('let value: &u32 = &42; value'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('mutable references report diagnostics and throw', () => {
  const invalid = () => rs`let value: &mut u32 = Box::leak(Box::new(42)); value`
  const error = diagnostics.find(message => message.includes('let value: &mut u32 = Box::leak'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('string slices report diagnostics and throw', () => {
  const invalid = () => rs`"Hello"`
  const error = diagnostics.find(message => message.includes('rs`"Hello"`'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('shared handle references report diagnostics and throw', () => {
  const message = rs`handy_crate::Message("Hello")`
  const invalid = () => rs`let shared: &handy_crate::Message = ${message}; shared`
  const error = diagnostics.find(message => message.includes('let shared: &handy_crate::Message'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('mutable handle references report diagnostics and throw', () => {
  const message = rs`handy_crate::Message("Hello")`
  const invalid = () => rs`let mutable: &mut handy_crate::Message = ${message}; mutable`
  const error = diagnostics.find(message => message.includes('let mutable: &mut handy_crate::Message'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('nested references report diagnostics and throw', () => {
  const invalid = () => rs`vec!["Hello"]`
  const error = diagnostics.find(message => message.includes('rs`vec!["Hello"]`'))

  expect(error).toMatch(/^E0308/)
  expect(error).toContain('JsValue')
  expect(invalid).toThrow(error)
})

test('multiple shared borrows work and conflicting borrows throw without deadlocking', () => {
  const message = rs`handy_crate::Message("Hello")`

  expect(rs`
    let first: &handy_crate::Message = ${message};
    let second: &handy_crate::Message = ${message};
    first.0 == second.0
  `).toBe(true)
  expect(() => rs`
    let first: &handy_crate::Message = ${message};
    let second: &mut handy_crate::Message = ${message};
    second.0 = first.0;
  `).toThrow(/borrow/i)
  expect(rs`let value: &handy_crate::Message = ${message}; value.0.to_owned()`).toBe('Hello')
})

test('reentrant calls cannot move a borrowed value and release locks after returning', () => {
  const message = rs`handy_crate::Message("Hello")`
  const take = () => rs`let value: handy_crate::Message = ${message}; value.0.to_owned()`
  let calls = 0
  const callback = () => {
    calls++
    expect(take).toThrow(/borrow/i)
  }

  rs`
    let message: &handy_crate::Message = ${message};
    let callback = ${callback};
    callback.apply(&[]).unwrap();
    assert_eq!(message.0, "Hello");
  `
  expect(calls).toBe(1)
  expect(take()).toBe('Hello')
})
