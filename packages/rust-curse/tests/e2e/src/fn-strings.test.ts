import { expect, it } from 'vitest'
import {
  accepts_mut_string,
  accepts_ref_string,
  accepts_string,
  identity_string,
  return_string,
  str_eq,
} from './lib.rs'

const utfValues = [
  'ascii',
  'café',
  'Здравствуйте',
  'こんにちは',
  'مرحبا',
  '🦀✨',
  'á',
]

it.each(utfValues)('accepts the str %s', (value) => {
  expect(str_eq(value, value)).toBe(true)
})

it('accepts an owned String', () => {
  expect(accepts_string('owned 🦀')).toBe(true)
})

it('accepts a referenced String', () => {
  expect(accepts_ref_string('borrowed 🦀')).toBe(true)
})

it('accepts a mutable referenced String', () => {
  expect(accepts_mut_string('mutable 🦀')).toBe(true)
})

it('returns an owned String', () => {
  expect(return_string()).toBe('returned 🦀')
})

it.each(utfValues)('round-trips the owned String %s', (value) => {
  expect(identity_string(value)).toBe(value)
})
