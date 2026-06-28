import { describe, expect, it } from 'vitest'
import { sum, sum_ref, sum_ref_mut } from './functions.rs'

describe('function', () => {
  it('called with two primitives', () => {
    console.log(sum)
    expect(sum(2, 3)).toBe(5)
  })
})
