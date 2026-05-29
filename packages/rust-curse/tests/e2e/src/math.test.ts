import { describe, expect, it } from 'vitest'
import { Point } from './math.rs'

describe('example', () => {
  it('point length', () => {
    const p = new Point(3, 0)
    expect(p.length()).toEqual(3)
  })
})
