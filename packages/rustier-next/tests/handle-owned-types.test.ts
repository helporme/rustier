import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('owned enums, collections, tuples, arrays, and boxes use handles', () => {
  const state = rs`handy_crate::State::Ready("ready".to_owned())`
  expect(rs`
    let state: &handy_crate::State = ${state};
    match state { handy_crate::State::Ready(text) => text.clone() }
  `).toBe('ready')

  const values = rs`vec![1u32, 2, 3]`
  rs`let values: &mut Vec<u32> = ${values}; values.push(4);`
  expect(rs`let values: Vec<u32> = ${values}; values.into_iter().sum::<u32>()`).toBe(10)
  expect(() => rs`let values: &Vec<u32> = ${values}; values.len() as u32`).toThrow(/moved/i)

  const tuple = rs`(42u32, "tuple".to_owned())`
  expect(rs`let value: &(u32, String) = ${tuple}; value.1.clone()`).toBe('tuple')
  const array = rs`[1u32, 2, 3]`
  expect(rs`let value: &[u32; 3] = ${array}; value[1]`).toBe(2)
  const boxed = rs`Box::new(42u32)`
  expect(rs`let value: Box<u32> = ${boxed}; *value`).toBe(42)
})

test('macro-local owned types can be returned as opaque handles', () => {
  const value = rs`struct Local(u32); Local(42)`

  expect(value.constructor.name).toBe('Rs')
})

test('handle inputs preserve public type aliases without naming implementation types', () => {
  const counter = rs`std::sync::atomic::AtomicU32::new(41)`

  expect(rs`
    let counter: &std::sync::atomic::AtomicU32 = ${counter};
    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
  `).toBe(41)
  expect(rs`
    let counter: std::sync::atomic::AtomicU32 = ${counter};
    counter.into_inner()
  `).toBe(42)
})
