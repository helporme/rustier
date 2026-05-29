import { describe, expect, it } from 'vitest'
import { Line, Point, Triangle } from './essential.rs'

describe('struct fields', () => {
  it('reads primitive fields', () => {
    const p = new Point(3, 4)
    expect(p.x).toBe(3)
    expect(p.y).toBe(4)
  })

  it('writes primitive fields', () => {
    const p = new Point(0, 0)
    p.x = 10
    p.y = -2
    expect(p.x).toBe(10)
    expect(p.y).toBe(-2)
  })

  // todo: check struct with all possible primitive fields including string
})

describe('constructors', () => {
  it('builds via `new`', () => {
    const p = new Point(3, 0)
    expect(p.length()).toBe(3)
  })

  it('builds via the generated `.of` factory', () => {
    const p = Point.of({ x: 3, y: 4 })
    expect(p.x).toBe(3)
    expect(p.y).toBe(4)
  })

  // todo: do not mapped to a method that accepts &self (it exists as static method)
  // todo: do not mapped to a method that accept &mut self (it exists as static method)
  // todo: do not mapped to a method that do not return Self (it exists as static method)
})

describe('inherent methods', () => {
  it('calls a `&self` method', () => {
    expect(new Point(3, 4).length()).toBe(5)
  })

  it('mutates through a `&mut self` method', () => {
    const p = new Point(3, 4)
    p.scale(2)
    expect(p.x).toBe(6)
    expect(p.y).toBe(8)
  })

  it('does not expose non-pub methods', () => {
    // `translate` is `pub(crate)`, so it must not cross to JS.
    expect('translate' in Point.prototype).toBe(false)
  })

  it('exposes methods from multiple inherent impls', () => {
    // `length`/`scale` live in the first impl block; both are reachable.
    const p = new Point(3, 4)
    expect(typeof p.length).toBe('function')
    expect(typeof p.scale).toBe('function')
  })
})

describe('the `of` collision', () => {
  it('keeps the user-defined `of` as a static method', () => {
    const line = Line.of(new Point(1, 2))
    expect(line.start.x).toBe(1)
    expect(line.end.x).toBe(1)
  })

  it('escapes the generated factory to `of$`', () => {
    const line = Line.of$({ start: { x: 1, y: 2 }, end: { x: 3, y: 4 } })
    expect(line.start.x).toBe(1)
    expect(line.end.x).toBe(3)
  })
})

describe('struct fields holding structs', () => {
  it('reads a nested struct field as a live value', () => {
    const line = Line.from(new Point(1, 2), new Point(3, 4))
    expect(line.start).toBeInstanceOf(Point)
    expect(line.start.x).toBe(1)
    expect(line.end.y).toBe(4)
  })

  it('reads two levels deep', () => {
    const t = Triangle.of({
      line: { start: { x: 1, y: 1 }, end: { x: 2, y: 2 } },
      point: { x: 9, y: 9 },
    })
    expect(t.line.start.x).toBe(1)
    expect(t.point.y).toBe(9)
  })

  // todo: setter isn't generated when field is not clonable, warning is emitted
  // todo: field isn't generated when field is not compatible, warning is emitted
})

describe('projection', () => {
  it('propagates a mutation on a nested field back to the parent', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    line.start.x = 42
    expect(line.start.x).toBe(42)
  })

  it('propagates a `&mut self` method called on a projected child', () => {
    const line = Line.from(new Point(3, 4), new Point(0, 0))
    line.start.scale(2)
    expect(line.start.x).toBe(6)
    expect(line.start.y).toBe(8)
  })

  it('detaches the previous child when a struct field is reassigned', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    const oldStart = line.start
    line.start = new Point(100, 200)
    // The handle taken before the write keeps its original value.
    expect(oldStart.x).toBe(1)
    expect(oldStart.y).toBe(1)
    // The new value is visible through the parent.
    expect(line.start.x).toBe(100)
    expect(line.start.y).toBe(200)
  })

  it('survives an aliased write without panicking', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    line.start = line.end
    expect(line.start.x).toBe(2)
    expect(line.end.x).toBe(2)
  })

  // todo: throws on mut aliasing
})

describe('coercion', () => {
  it('coerces object literals in a static method param', () => {
    const line = Line.from({ x: 1, y: 2 }, { x: 3, y: 4 })
    expect(line.start.x).toBe(1)
    expect(line.end.y).toBe(4)
  })

  it('accepts a live class instance in the same param', () => {
    const line = Line.from(new Point(5, 6), { x: 7, y: 8 })
    expect(line.start.x).toBe(5)
    expect(line.end.x).toBe(7)
  })

  it('coerces recursively through nested struct fields', () => {
    const t = Triangle.of({
      line: { start: { x: 0, y: 0 }, end: { x: 1, y: 1 } },
      point: { x: 2, y: 3 },
    })
    expect(t.line.end.x).toBe(1)
    expect(t.point.x).toBe(2)
  })
})

// todo: unit stucts emit warning and not generated
// todo: tuple stucts emit warning and not generated

// todo: struct that have a pub field of another SKIPPED struct is SKIPPED
// todo: function that accepts as a argument another SKIPPEd struct is SKIPPED

// todo: function: non-public fns are not accessable
// todo: function can accept any primitive by value
// todo: function can return any primitive by value
// todo: function cannot accept any primitive by ref / mut ref and it emits warn and function is skipped
// todo: function passing a struct by ref doesn't clone it
// todo: function passing a struct by value does clone it
// todo: function can return any struct and it is passed by ref

// next: analyze what values can clash
