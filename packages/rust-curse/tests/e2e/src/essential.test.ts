import { describe, expect, it } from 'vitest'
import * as essential from './essential.rs'
import {
  Line,
  MutSelfNew,
  Point,
  Primitives,
  SelfNew,
  StaticNew,
  Triangle,
  Wrapper,
  add,
  inc,
  nudge,
  origin,
  read_ref,
  shift_copy,
  shout,
} from './essential.rs'
import { hasWarn } from './utils'

describe('struct fields', () => {
  it('reads every primitive field', () => {
    const p = Primitives.of({
      flag: true,
      text: 'hi',
      sbyte: -5,
      byte: 200,
      short: -1000,
      ushort: 60000,
      int: -100000,
      uint: 100000,
      big: 2.5,
    })
    expect(p.flag).toBe(true)
    expect(p.text).toBe('hi')
    expect(p.sbyte).toBe(-5)
    expect(p.byte).toBe(200)
    expect(p.short).toBe(-1000)
    expect(p.ushort).toBe(60000)
    expect(p.int).toBe(-100000)
    expect(p.uint).toBe(100000)
    expect(p.big).toBe(2.5)
  })

  it('writes every primitive field', () => {
    const p = Primitives.of({
      flag: false,
      text: '',
      sbyte: 0,
      byte: 0,
      short: 0,
      ushort: 0,
      int: 0,
      uint: 0,
      big: 0,
    })
    p.flag = true
    p.text = 'world'
    p.sbyte = -1
    p.byte = 255
    p.short = 1234
    p.ushort = 4321
    p.int = -9
    p.uint = 9
    p.big = 123.25
    expect(p.flag).toBe(true)
    expect(p.text).toBe('world')
    expect(p.sbyte).toBe(-1)
    expect(p.byte).toBe(255)
    expect(p.short).toBe(1234)
    expect(p.ushort).toBe(4321)
    expect(p.int).toBe(-9)
    expect(p.uint).toBe(9)
    expect(p.big).toBe(123.25)
  })
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

  it('keeps a `new` that takes `&self` as an instance method', () => {
    // `new(&self)` has a receiver, so it can't be the JS constructor.
    expect('new' in SelfNew.prototype).toBe(true)
    const a = SelfNew.of({ v: 7 })
    const b = a.new()
    expect(b.v).toBe(7)
    expect(b).not.toBe(a)
  })

  it('keeps a `new` that takes `&mut self` as an instance method', () => {
    expect('new' in MutSelfNew.prototype).toBe(true)
    const a = MutSelfNew.of({ v: 1 })
    const b = a.new()
    expect(a.v).toBe(2)
    expect(b.v).toBe(2)
  })

  it('keeps a `new` that does not return Self as a static method', () => {
    // Returns a number, so it stays static rather than becoming the ctor.
    expect('new' in StaticNew.prototype).toBe(false)
    expect(StaticNew.new(3)).toBe(6)
    // It's still constructible through the generated factory.
    expect(StaticNew.of({ v: 5 }).v).toBe(5)
  })
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

  it('suppresses the setter when the field is not clonable, with a warning', () => {
    const w = new Wrapper(5)
    expect(w.inner.v).toBe(5)
    // `NoClone` has no `Clone`, so the field is read-only.
    expect(() => {
      // @ts-expect-error inner is readonly (no setter generated)
      w.inner = new (essential as any).NoClone(9)
    }).toThrow()
    expect(
      hasWarn('rust-curse::field_setter_unavailable', 'Wrapper.inner'),
    ).toBe(true)
  })

  it('skips a struct with an incompatible field, with a warning', () => {
    // `BadField { items: Vec<f64> }` -> unsupported type -> not generated.
    expect((essential as any).BadField).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'Vec')).toBe(true)
  })

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

  // todo: currently panics on node
  it.skip('throws on a mutably-aliased method call', () => {
    // Both children project from the same parent; a `&mut self` call that also
    // borrows the parent through `other` double-borrows the shared cell.
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    expect(() => line.start.add_assign(line.end)).toThrow()
  })

  it('does not throw when the receiver and arg are independent', () => {
    const a = new Point(1, 1)
    const b = new Point(2, 3)
    a.add_assign(b)
    expect(a.x).toBe(3)
    expect(a.y).toBe(4)
  })
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

describe('skipped structs', () => {
  it('skips unit structs, with a warning', () => {
    expect((essential as any).Marker).toBeUndefined()
    expect(
      hasWarn(
        'rust-curse::unsupported_type',
        'unit structs are not yet supported',
      ),
    ).toBe(true)
  })

  it('skips tuple structs, with a warning', () => {
    expect((essential as any).Pair).toBeUndefined()
    expect(
      hasWarn(
        'rust-curse::unsupported_type',
        'tuple structs are not yet supported',
      ),
    ).toBe(true)
  })

  it('skips a struct with a pub field of a skipped struct', () => {
    // `Holder { mark: Marker }` references the skipped unit struct.
    expect((essential as any).Holder).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Marker')).toBe(true)
  })

  it('skips a function that accepts a skipped struct', () => {
    // `use_pair(p: Pair)` references the skipped tuple struct.
    expect((essential as any).use_pair).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Pair')).toBe(true)
  })
})

describe('functions', () => {
  it('does not expose non-pub functions', () => {
    expect((essential as any).private_fn).toBeUndefined()
  })

  it('accepts and returns primitives by value', () => {
    expect(add(2, 3)).toBe(5)
    expect(shout('hi')).toBe('HI')
  })

  it('accepts a primitive by ref, with no warning', () => {
    expect(read_ref(5)).toBe(5)
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"read_ref"')).toBe(
      false,
    )
  })

  it('accepts a primitive by mut ref, but warns the write is dropped', () => {
    // The call works, but JS passed the number by value so the `&mut` write
    // can't propagate back — there's nothing to observe, only the warning.
    expect(() => inc(5)).not.toThrow()
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"inc"')).toBe(true)
  })

  it('passes a struct by mut ref without cloning it', () => {
    const p = new Point(1, 2)
    nudge(p, 5)
    expect(p.x).toBe(6)
    expect(p.y).toBe(7)
  })

  it('clones a struct passed by value', () => {
    const a = new Point(1, 2)
    const b = shift_copy(a, 10)
    expect(b.x).toBe(11)
    expect(b.y).toBe(2)
    // The original is untouched because it was cloned at the boundary.
    expect(a.x).toBe(1)
  })

  it('returns a struct as a live handle', () => {
    const o = origin()
    expect(o).toBeInstanceOf(Point)
    expect(o.x).toBe(0)
    o.x = 5
    expect(o.x).toBe(5)
  })
})

// todo: test {...point} and JSON.stringify(point)
// next: analyze implementation to find what args/field names can potentially clash with internal codegen values, test it
// next: use all possible js keywords as struct field names to find how it can break things
// next: exercise nested module, pub use my::* as X and simillar.
