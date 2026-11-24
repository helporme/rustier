import { expect, test } from 'vitest'
import { rs } from 'rustier'

test('recognizes only genuine handles of the requested Rust type', () => {
  const message = rs`handy_crate::Message("hello")`
  const isMessage = (input: unknown) => rs`
    let value: JsValue = ${input};
    value.is_rust_handle::<handy_crate::Message>()
  `
  expect(isMessage(message)).toBe(true)
  expect(isMessage(rs`vec![42u32]`)).toBe(false)
  for (const other of [null, 1n, {}, new Proxy(message, {}), Object.create(message),
    { constructor: { name: 'Rs' }, ' __UNSAFE_ptr': message[' __UNSAFE_ptr'] },
    { get [' __UNSAFE_ptr']() { throw new Error('unexpected pointer read') } },
  ]) {
    expect(isMessage(other)).toBe(false)
  }
})

test('a shared borrow allows another reader but blocks mutation and take', () => {
  const item = rs`handy_crate::CopyValue(41)`
  expect(rs`
    let value: JsValue = ${item};
    let first: RsValueHandle<handy_crate::CopyValue> = value.as_rust_handle().unwrap();
    let second = value.as_rust_handle::<handy_crate::CopyValue>().unwrap();
    let shared = first.borrow().unwrap();
    assert_eq!(second.borrow().unwrap().0, shared.0);
    assert!(second.borrow_mut().is_none() && second.take().is_none());
    shared.0
  `).toBe(41)
})

test('an exclusive borrow blocks aliases until the mutation is released', () => {
  const item = rs`handy_crate::CopyValue(41)`
  expect(rs`
    let value: JsValue = ${item};
    let first = value.as_rust_handle::<handy_crate::CopyValue>().unwrap();
    let mut exclusive = first.borrow_mut().unwrap();
    assert!(value.is_rust_handle::<handy_crate::CopyValue>());
    let second = value.as_rust_handle::<handy_crate::CopyValue>().unwrap();
    assert!(second.borrow().is_none() && second.borrow_mut().is_none() && second.take().is_none());
    exclusive.0 += 1;
    drop(exclusive);
    second.borrow().unwrap().0
  `).toBe(42)
})

test('taking a value moves it once across all aliases without changing its handle type', () => {
  const item = rs`handy_crate::CopyValue(41)`
  expect(rs`
    let value: JsValue = ${item};
    let first = value.as_rust_handle::<handy_crate::CopyValue>().unwrap();
    let second = value.as_rust_handle::<handy_crate::CopyValue>().unwrap();
    drop(value);
    let result = second.take().unwrap();
    assert!(first.borrow().is_none() && first.borrow_mut().is_none() && first.take().is_none());
    result.0
  `).toBe(41)
  expect(rs`
    let value: JsValue = ${item};
    value.is_rust_handle::<handy_crate::CopyValue>()
  `).toBe(true)
  expect(() => rs`let v: handy_crate::CopyValue = ${item}; v.0`).toThrow(/moved/i)
})
