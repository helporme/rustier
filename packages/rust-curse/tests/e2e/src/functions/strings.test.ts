import { describe, expect, it } from 'vitest'
import {
  accepts_mut_string,
  accepts_qualified_string,
  accepts_ref_string,
  accepts_string,
  identity_string,
  return_string,
} from './strings.rs'

describe('string functions', () => {
  it('accepts owned String parameters', () => {
    expect(accepts_string('owned 🦀')).toBe(true)
    expect(accepts_qualified_string('qualified 🦀')).toBe(true)
  })

  it('accepts referenced String parameters', () => {
    expect(accepts_ref_string('borrowed 🦀')).toBe(true)
  })

  it('accepts mutable String parameters', () => {
    expect(accepts_mut_string('mutable 🦀')).toBe(true)
  })

  it('returns owned String values', () => {
    expect(return_string()).toBe('returned 🦀')
  })

  it('round-trips owned String values', () => {
    const values = [
      'ascii',
      'café',
      'Здравствуйте',
      'こんにちは',
      'مرحبا',
      '🦀✨',
      'á',
    ]

    for (const value of values) {
      expect(identity_string(value)).toBe(value)
    }
  })
})
