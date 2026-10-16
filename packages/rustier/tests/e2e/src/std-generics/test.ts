import { expect, it } from 'vitest'
import { Vec } from '$rust/alloc/vec'
import { Option } from '$rust/core/option'
import { Result } from '$rust/core/result'
import {
  checked_number,
  nested_word_count,
  nested_words,
  optional_number,
  word_count,
  words,
} from './test.rs'

it('reads and constructs both Option<i32> variants through the standard library template', () => {
  const OptionalNumber = Option('i32')
  const some = optional_number(true)
  const none = optional_number(false)

  expect(some).toBeInstanceOf(OptionalNumber)
  expect(none).toBeInstanceOf(OptionalNumber)
  expect(some.type).toBe('Some')
  if (some.type !== 'Some') throw new Error('expected Option::Some')
  expect(some[0]).toBe(42)
  expect(none.type).toBe('None')
  expect(OptionalNumber.of({ type: 'Some', 0: 7 }).is_some()).toBe(true)
  expect(OptionalNumber.of({ type: 'None' }).is_none()).toBe(true)
})

it('constructs Rust Result enum values and unwraps a returned Ok', () => {
  const CheckedNumber = Result('i32', 'String')
  const ok = CheckedNumber.of({ type: 'Ok', 0: 42 })
  const error = CheckedNumber.of({ type: 'Err', 0: 'invalid' })

  expect(checked_number(true)).toBe(42)
  expect(ok).toBeInstanceOf(CheckedNumber)
  expect(error).toBeInstanceOf(CheckedNumber)
  expect(ok.type).toBe('Ok')
  if (ok.type !== 'Ok') throw new Error('expected Result::Ok')
  expect(ok[0]).toBe(42)
  expect(error.type).toBe('Err')
  if (error.type !== 'Err') throw new Error('expected Result::Err')
  expect(error[0]).toBe('invalid')
  expect(CheckedNumber.of({ type: 'Err', 0: '' }).is_err()).toBe(true)
})

it('mutates a Vec<String> and reads its Option<String> pop result', () => {
  const Strings = Vec('String')
  const values = words()
  const empty = new Strings()

  expect(values).toBeInstanceOf(Strings)
  expect(word_count(empty)).toBe(0)
  values.push('🦀')
  expect(word_count(values)).toBe(2)
  const last = values.pop()

  expect(last.type).toBe('Some')
  if (last.type !== 'Some') throw new Error('expected Option::Some')
  expect(last[0]).toBe('🦀')
  expect(word_count(values)).toBe(1)
  expect(empty.pop().type).toBe('None')
})

it('clones inner vectors when passing them by value into a nested Vec', () => {
  const Strings = Vec('String')
  const Groups = Vec(Strings)
  const inner = new Strings()
  const outer = nested_words()

  inner.push('second')
  expect(outer).toBeInstanceOf(Groups)
  outer.push(inner)
  inner.push('not in outer')

  expect(nested_word_count(outer)).toBe(2)
  expect(word_count(inner)).toBe(2)
  const group = outer.pop()

  expect(group.type).toBe('Some')
  if (group.type !== 'Some') throw new Error('expected Option::Some')
  expect(group[0]).toBeInstanceOf(Strings)
  expect(word_count(group[0])).toBe(1)
  expect(nested_word_count(outer)).toBe(1)
})
