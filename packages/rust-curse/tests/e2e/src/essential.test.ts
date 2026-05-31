import { describe, expect, it } from 'vitest'
import dtsText from '../.rust-curse/types/src/essential.rs.d.ts?raw'
import * as essential from './essential.rs'
import {
  Adder,
  Bigs,
  Both,
  BuildOutput,
  Builder,
  CamelTags,
  CloneCounted,
  Config,
  DerivedExtras,
  Element,
  Empty,
  EnumWithNew,
  EnumWithOf,
  Event,
  Hazard,
  Hitbox,
  Inner,
  Keywords,
  Line,
  Maker,
  ManualClone,
  Mesh,
  Mixed,
  MutSelfNew,
  Named,
  NewAndOf,
  NoClone,
  NonCloneState,
  One,
  Opaque,
  Outer,
  Point,
  Primitives,
  Quad,
  Reenter,
  SelfNew,
  Shadow,
  SharedOk,
  SnakeCase,
  Span,
  StaticNew,
  Triangle,
  TwoNew,
  UnitMix,
  Wrapper,
  Zero,
  add,
  append,
  boom,
  bump_cc,
  enum_clone_count,
  force_off,
  inc,
  label,
  make_cc,
  many,
  nudge,
  old,
  origin,
  point,
  read_cc,
  read_ref,
  reset_enum_clones,
  shift_copy,
  shout,
  take_cc,
  tick,
  use_inner,
} from './essential.rs'
import { hasWarn, makeBigInt, target } from './utils'

const primitiveInit = (): Primitives.Init => ({
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

describe('primitive fields', () => {
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
    const p = Primitives.of(primitiveInit())
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

  it('float into integer field truncates', () => {
    const p = Primitives.of(primitiveInit())
    p.int = 2.5
    expect(p.int).toBe(2)
  })

  it('f64 special values survive', () => {
    const p = Primitives.of(primitiveInit())
    p.big = Number.NaN
    expect(Number.isNaN(p.big)).toBe(true)
    p.big = Number.POSITIVE_INFINITY
    expect(p.big).toBe(Number.POSITIVE_INFINITY)
    p.big = -0
    expect(Object.is(p.big, -0)).toBe(true)
    p.big = Number.MAX_SAFE_INTEGER
    expect(p.big).toBe(Number.MAX_SAFE_INTEGER)
  })

  it('empty, long, and unicode strings round-trip', () => {
    const p = Primitives.of(primitiveInit())
    for (const value of ['', 'x'.repeat(4096), 'Zażółć 😀 東京']) {
      p.text = value
      expect(p.text).toBe(value)
      expect(shout(value)).toBe(value.toUpperCase())
    }
  })

  it('string with embedded NUL round-trips', () => {
    const p = Primitives.of(primitiveInit())
    p.text = 'a\0b'
    expect(p.text).toBe('a\0b')
  })

  it('unit return is undefined', () => {
    expect(tick()).toBeUndefined()
  })
})

describe('constructors', () => {
  it('new(x, y) is the JS constructor', () => {
    expect(new Point(3, 4).length()).toBe(5)
  })

  it('builds via the generated .of factory', () => {
    const p = Point.of({ x: 3, y: 4 })
    expect(p.x).toBe(3)
    expect(p.y).toBe(4)
  })

  it('&self new stays an instance method', () => {
    expect('new' in SelfNew.prototype).toBe(true)
    const a = SelfNew.of({ v: 7 })
    const b = a.new()
    expect(b.v).toBe(7)
    expect(b).not.toBe(a)
  })

  it('&mut self new stays an instance method', () => {
    expect('new' in MutSelfNew.prototype).toBe(true)
    const a = MutSelfNew.of({ v: 1 })
    const b = a.new()
    expect(a.v).toBe(2)
    expect(b.v).toBe(2)
  })

  it('non-Self new stays static', () => {
    expect('new' in StaticNew.prototype).toBe(false)
    expect(StaticNew.new(3)).toBe(6)
    expect(StaticNew.of({ v: 5 }).v).toBe(5)
  })

  it('zero-arg new() -> Self builds', () => {
    expect(new Zero().value).toBe(0)
  })

  it('multiple new candidates deterministically pick the inherent constructor', () => {
    expect(new TwoNew(5).v).toBe(5)
  })

  it('new with a struct param coerces object literals and instances', () => {
    const span = new Span({ x: 1, y: 2 }, new Point(3, 4))
    expect(span.start.x).toBe(1)
    expect(span.end.y).toBe(4)
  })

  it('named factory is static, not the constructor', () => {
    expect('create' in Maker).toBe(true)
    expect(Maker.create(7).v).toBe(7)
  })

  it('struct-expressible struct with real new has both constructor and factory', () => {
    expect(new Both().value).toBe(42)
    expect(Both.of({ value: 7 }).value).toBe(7)
  })
})

describe('inherent methods', () => {
  it('&self method reads', () => {
    expect(new Point(3, 4).length()).toBe(5)
  })

  it('&mut self method mutates', () => {
    const p = new Point(3, 4)
    p.scale(2)
    expect(p.x).toBe(6)
    expect(p.y).toBe(8)
  })

  it('pub(crate) method is not exposed', () => {
    expect('translate' in Point.prototype).toBe(false)
  })

  it('methods from two impls are both reachable', () => {
    const p = new Point(3, 4)
    expect(typeof p.length).toBe('function')
    expect(typeof p.scale).toBe('function')
    expect(typeof p.add_assign).toBe('function')
  })

  it('self-by-value on Clone keeps original usable', () => {
    const p = new Point(1, 2)
    const out = p.consume()
    expect(out.x).toBe(1)
    p.x = 9
    expect(out.x).toBe(1)
  })

  it('self-by-value on non-Clone is dropped', () => {
    expect('consume' in NoClone.prototype).toBe(false)
    expect(
      hasWarn(
        'rust-curse::method_self_by_value_requires_clone',
        'NoClone::consume',
      ),
    ).toBe(true)
  })

  it('method returning Self chains', () => {
    expect(new Builder(1).with(2).with(3).v).toBe(6)
  })

  it('method returning another struct returns a usable handle', () => {
    const other = new Point(3, 4).to_origin()
    expect(other.value).toBe(5)
  })

  it('method with multiple struct params accepts literals and instances', () => {
    const p = new Point(1, 1).combine(new Point(2, 3), new Point(4, 5))
    expect(p.x).toBe(7)
    expect(p.y).toBe(9)
  })

  it('method name shadowing field is dropped with a warning', () => {
    const s = Shadow.of({ len: 4 })
    expect(s.len).toBe(4)
    expect(typeof (s as unknown as { len: unknown }).len).toBe('number')
    expect(hasWarn('rust-curse::method_shadows_field', 'Shadow::len')).toBe(
      true,
    )
  })

  it('JS protocol-named methods are callable', () => {
    const h = Hazard.of({ value: 2, then: 3 })
    expect(h.toString()).toBe('Hazard(2, 3)')
    expect(h.valueOf()).toBe(5)
  })
})

describe('struct projection', () => {
  it('reads a nested field as a live handle', () => {
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

  it('mutating a nested field propagates', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    line.start.x = 42
    expect(line.start.x).toBe(42)
  })

  it('&mut self method on a projected child propagates', () => {
    const line = Line.from(new Point(3, 4), new Point(0, 0))
    line.start.scale(2)
    expect(line.start.x).toBe(6)
    expect(line.start.y).toBe(8)
  })

  it('mutates two levels deep', () => {
    const t = Triangle.of({
      line: { start: { x: 1, y: 1 }, end: { x: 2, y: 2 } },
      point: { x: 9, y: 9 },
    })
    t.line.start.x = 10
    expect(t.line.start.x).toBe(10)
  })

  it('mutates three levels deep', () => {
    const q = Quad.of({
      tri: {
        line: { start: { x: 1, y: 1 }, end: { x: 2, y: 2 } },
        point: { x: 3, y: 3 },
      },
    })
    q.tri.line.start.y = 11
    expect(q.tri.line.start.y).toBe(11)
  })

  it('reassigning a field detaches old handle', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    const oldStart = line.start
    line.start = new Point(100, 200)
    expect(oldStart.x).toBe(1)
    expect(oldStart.y).toBe(1)
    expect(line.start.x).toBe(100)
    expect(line.start.y).toBe(200)
  })

  it('two handles to the same child share mutation', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    const a = line.start
    const b = line.start
    expect(a).not.toBe(b)
    a.x = 8
    expect(b.x).toBe(8)
  })

  it('projected child into &mut Point fn mutates parent', () => {
    const line = Line.from(new Point(1, 2), new Point(3, 4))
    nudge(line.start, 5)
    expect(line.start.x).toBe(6)
    expect(line.start.y).toBe(7)
  })

  it('projected child by value into fn clones and leaves parent untouched', () => {
    const line = Line.from(new Point(1, 2), new Point(3, 4))
    const shifted = shift_copy(line.start, 10)
    expect(shifted.x).toBe(11)
    expect(line.start.x).toBe(1)
  })

  it('child handle remains safe after parent reassign', () => {
    const line = Line.from(new Point(1, 2), new Point(3, 4))
    const child = line.start
    line.start = new Point(9, 9)
    child.scale(2)
    expect(child.x).toBe(2)
    expect(line.start.x).toBe(9)
  })

  it('reassign field from object literal is rejected', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    expect(() => {
      line.start = { x: 7, y: 8 } as Point
    }).toThrow()
  })

  it('aliased write does not panic', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    line.start = line.end
    expect(line.start.x).toBe(2)
    expect(line.end.x).toBe(2)
  })

  it.skip('mutably-aliased method call', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    expect(() => line.start.add_assign(line.end)).toThrow()
  })

  it('independent receiver+arg method works', () => {
    const a = new Point(1, 1)
    const b = new Point(2, 3)
    a.add_assign(b)
    expect(a.x).toBe(3)
    expect(a.y).toBe(4)
  })

  it('non-clonable field is read-only', () => {
    const w = new Wrapper(5)
    expect(w.inner.v).toBe(5)
    expect(() => {
      ;(w as { inner: NoClone }).inner = new NoClone(9)
    }).toThrow()
    expect(
      hasWarn('rust-curse::field_setter_unavailable', 'Wrapper.inner'),
    ).toBe(true)
  })

  it('mutates non-clonable child via method', () => {
    const w = new Wrapper(5)
    w.inner.bump(3)
    expect(w.inner.v).toBe(8)
  })
})

describe('coercion', () => {
  it('coerces object literals in a static method param', () => {
    const line = Line.from({ x: 1, y: 2 }, { x: 3, y: 4 })
    expect(line.start.x).toBe(1)
    expect(line.end.y).toBe(4)
  })

  it('accepts a live instance in the same param', () => {
    const line = Line.from(new Point(5, 6), { x: 7, y: 8 })
    expect(line.start.x).toBe(5)
    expect(line.end.x).toBe(7)
  })

  it('recursive coercion through nesting', () => {
    const t = Triangle.of({
      line: { start: { x: 0, y: 0 }, end: { x: 1, y: 1 } },
      point: { x: 2, y: 3 },
    })
    expect(t.line.end.x).toBe(1)
    expect(t.point.x).toBe(2)
  })

  it('coerces in a constructor param', () => {
    const span = new Span({ x: 1, y: 2 }, { x: 3, y: 4 })
    expect(span.end.x).toBe(3)
  })

  it('coerces in a free function param', () => {
    const shifted = shift_copy({ x: 1, y: 2 }, 5)
    expect(shifted.x).toBe(6)
  })

  it('extra keys in a literal are ignored', () => {
    const p = Point.of({ x: 1, y: 2, z: 3 } as Point.Init)
    expect(p.x).toBe(1)
    expect(p.y).toBe(2)
  })

  it('missing key in a literal throws', () => {
    expect(() => Point.of({ x: 1 } as Point.Init)).toThrow()
  })

  it('wrong-typed value in a literal throws', () => {
    expect(() =>
      Point.of({ x: 1, y: 'two' } as unknown as Point.Init),
    ).toThrow()
  })

  it('null/undefined for a struct param throws', () => {
    expect(() => shift_copy(null as unknown as Point, 1)).toThrow()
    expect(() => shift_copy(undefined as unknown as Point, 1)).toThrow()
  })

  it('primitive/array for a struct param throws', () => {
    expect(() => shift_copy(1 as unknown as Point, 1)).toThrow()
    expect(() => shift_copy([] as unknown as Point, 1)).toThrow()
  })

  it('same literal to two params produces independent clones', () => {
    const init = { x: 1, y: 2 }
    const line = Line.from(init, init)
    line.start.x = 9
    expect(line.end.x).toBe(1)
  })

  it('getter-bearing literal invokes each getter once', () => {
    let xReads = 0
    let yReads = 0
    const init = {
      get x() {
        xReads += 1
        return 1
      },
      get y() {
        yReads += 1
        return 2
      },
    }
    expect(Point.of(init).y).toBe(2)
    expect(xReads).toBe(1)
    expect(yReads).toBe(1)
  })

})

describe('functions', () => {
  it('non-pub fn not exported', () => {
    expect((essential as Record<string, unknown>).private_fn).toBeUndefined()
  })

  it('primitives by value', () => {
    expect(add(2, 3)).toBe(5)
    expect(shout('hi')).toBe('HI')
  })

  it('primitive by & ref works, no warning', () => {
    expect(read_ref(5)).toBe(5)
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"read_ref"')).toBe(
      false,
    )
  })

  it('primitive by &mut ref warns and write is dropped', () => {
    expect(() => inc(5)).not.toThrow()
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"inc"')).toBe(true)
  })

  it('struct by &mut mutates', () => {
    const p = new Point(1, 2)
    nudge(p, 5)
    expect(p.x).toBe(6)
    expect(p.y).toBe(7)
  })

  it('struct by value clones', () => {
    const a = new Point(1, 2)
    const b = shift_copy(a, 10)
    expect(b.x).toBe(11)
    expect(a.x).toBe(1)
  })

  it('returns a struct as a live handle', () => {
    const o = origin()
    expect(o).toBeInstanceOf(Point)
    o.x = 5
    expect(o.x).toBe(5)
  })

  it('&mut String param warns and write is dropped', () => {
    expect(() => append('hi')).not.toThrow()
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"append"')).toBe(
      true,
    )
  })

  it('owned String + struct by-ref works', () => {
    const p = new Point(1, 2)
    expect(label(p, 'abc')).toBe('abc')
    expect(p.x).toBe(4)
  })

  it('many params preserve order', () => {
    expect(many(1, 2, 3, 4, 5, 6)).toBe(654321)
  })

  it('too many JS args are ignored', () => {
    expect((add as (...args: number[]) => number)(1, 2, 3)).toBe(3)
  })

  it.skip('Rust panic surfaces as a throw', () => {
    expect(() => boom()).toThrow()
  })
})

describe('name escaping', () => {
  it('user of kept as static method', () => {
    const line = Line.of(new Point(1, 2))
    expect(line.start.x).toBe(1)
    expect(line.end.x).toBe(1)
  })

  it('generated factory escaped to of$', () => {
    const line = Line.of$({ start: { x: 1, y: 2 }, end: { x: 3, y: 4 } })
    expect(line.start.x).toBe(1)
    expect(line.end.x).toBe(3)
  })

  it('new and of both user-defined keep both behaviors', () => {
    expect(new NewAndOf(2).value).toBe(2)
    expect(NewAndOf.of(2)).toBe(3)
    expect(NewAndOf.of$({ value: 4 }).value).toBe(4)
  })

  it('field named then is not thenable', async () => {
    const h = Hazard.of({ value: 1, then: 2 })
    await expect(Promise.resolve(h)).resolves.toBe(h)
  })

  it('reserved-word fields are accessible', () => {
    const k = Keywords.of({ class: 1, delete: 2 })
    expect(k.class).toBe(1)
    expect(k.delete).toBe(2)
  })

  it('snake_case emit is pinned', () => {
    const s = SnakeCase.of({ my_field: 1 })
    expect(s.my_field).toBe(1)
    expect('myField' in s).toBe(false)
  })
})

describe('skipped and unsupported', () => {
  it('unit struct skipped', () => {
    expect((essential as Record<string, unknown>).Marker).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'unit structs')).toBe(true)
  })

  it('tuple struct skipped', () => {
    expect((essential as Record<string, unknown>).Pair).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'tuple structs')).toBe(true)
  })

  it('field of a skipped struct cascades', () => {
    expect((essential as Record<string, unknown>).Holder).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Marker')).toBe(true)
  })

  it('fn param of a skipped struct cascades', () => {
    expect((essential as Record<string, unknown>).use_pair).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Pair')).toBe(true)
  })

  it('Vec field skips struct', () => {
    expect((essential as Record<string, unknown>).BadField).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'Vec')).toBe(true)
  })

  it('all-unit enum skipped', () => {
    expect((essential as Record<string, unknown>).Color).toBeUndefined()
    expect(
      hasWarn('rust-curse::enum_all_unit_unsupported', 'all-unit'),
    ).toBe(true)
  })

  it('fn returning unsupported type is absent', () => {
    expect((essential as Record<string, unknown>).bad_return).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'Vec')).toBe(true)
  })

  it('fn returning skipped struct is absent', () => {
    expect((essential as Record<string, unknown>).make_pair).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Pair')).toBe(true)
  })

  it('generic struct and function are dropped', () => {
    expect((essential as Record<string, unknown>).Generic).toBeUndefined()
    expect((essential as Record<string, unknown>).id).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'Generic("T")')).toBe(true)
  })

  it('slice/array/tuple fields skip structs', () => {
    expect((essential as Record<string, unknown>).SliceField).toBeUndefined()
    expect((essential as Record<string, unknown>).ArrayField).toBeUndefined()
    expect((essential as Record<string, unknown>).TupleField).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_type', 'slices')).toBe(true)
    expect(hasWarn('rust-curse::unsupported_type', 'fixed-size arrays')).toBe(
      true,
    )
    expect(hasWarn('rust-curse::unsupported_type', 'tuples')).toBe(true)
  })

  it('external-crate type skips struct and warns', () => {
    expect((essential as Record<string, unknown>).Extern).toBeUndefined()
    expect(hasWarn('rust-curse::unsupported_import', 'HashMap')).toBe(true)
  })
})

describe('traits and derives', () => {
  it('trait-impl methods are not exposed', () => {
    expect('fmt' in Named.prototype).toBe(false)
  })

  it('operator-trait methods are not exposed', () => {
    expect('add' in Adder.prototype).toBe(false)
  })

  it('derives beyond Clone are ignored and struct still emits', () => {
    expect(DerivedExtras.of({ v: 1 }).v).toBe(1)
  })

  it('manual Clone enables setters and factory', () => {
    const m = ManualClone.of({ v: 1 })
    m.v = 2
    expect(m.v).toBe(2)
  })

  it('derive Default has no default static method', () => {
    expect('default' in Config).toBe(false)
    expect(Config.of({ enabled: true }).enabled).toBe(true)
  })
})

describe('object behavior', () => {
  it('instanceof generated class is true', () => {
    expect(new Point(1, 2)).toBeInstanceOf(Point)
  })

  it('identity is normal JS identity', () => {
    const p = new Point(1, 2)
    expect(p).toBe(p)
    expect(Point.of({ x: 1, y: 2 })).not.toBe(Point.of({ x: 1, y: 2 }))
  })

  it('delete point.x is rejected', () => {
    const p = new Point(1, 2)
    expect(delete (p as Partial<Point>).x).toBe(true)
    expect(p.x).toBe(1)
  })

  it('set unknown property sticks as a JS property', () => {
    const p = new Point(1, 2) as Point & { z?: number }
    p.z = 3
    expect(p.z).toBe(3)
  })

  it('toString/template literal output is pinned', () => {
    const p = new Point(1, 2)
    expect(p.toString()).toBe('[object Object]')
    expect(`${p}`).toBe('[object Object]')
  })
})

describe('memory and lifetimes', () => {
  it('many instances created and dropped', () => {
    for (let i = 0; i < 1000; i++) {
      expect(new Point(i, i + 1).x).toBe(i)
    }
  })

  it('projected child past parent reassign is safe', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    const child = line.start
    line.start = new Point(3, 3)
    child.x = 9
    expect(child.x).toBe(9)
    expect(line.start.x).toBe(3)
  })

  it('re-entrant &mut self method completes', () => {
    const r = new Reenter(1)
    r.outer()
    expect(r.value).toBe(2)
  })

  it.skip('p.add_assign(p) aliased receiver+arg is pinned', () => {
    const p = new Point(1, 2)
    expect(() => p.add_assign(p)).toThrow()
  })

  it('long projection chain then mutate root invalidates caches', () => {
    const q = Quad.of({
      tri: {
        line: { start: { x: 1, y: 1 }, end: { x: 2, y: 2 } },
        point: { x: 3, y: 3 },
      },
    })
    const start = q.tri.line.start
    q.tri.line = Line.from(new Point(5, 5), new Point(6, 6))
    expect(start.x).toBe(5)
    expect(q.tri.line.start.x).toBe(5)
  })

  it('rapid repeated field reassign detaches old handles', () => {
    const line = Line.from(new Point(0, 0), new Point(1, 1))
    const seen: Point[] = []
    for (let i = 0; i < 10; i++) {
      seen.push(line.start)
      line.start = new Point(i, i)
    }
    seen[0].x = 99
    expect(line.start.x).toBe(9)
  })
})

describe('target differences', () => {
  it('i64 field runtime type differs from declared bigint on node', () => {
    const max = makeBigInt('9223372036854775807')
    if (target === 'wasm') {
      const b = Bigs.of({ signed: max })
      expect(b.signed).toBe(max)
      b.signed = -max
      expect(b.signed).toBe(-max)
    } else {
      expect(() => Bigs.of({ signed: max })).toThrow()
      const b = Bigs.of({ signed: 42 as unknown as bigint })
      expect(b.signed).toBe(42)
      b.signed = -7 as unknown as bigint
      expect(b.signed).toBe(-7)
    }
  })

  it('narrow-int overflow wraps on wasm and throws on node', () => {
    const p = Primitives.of(primitiveInit())
    if (target === 'wasm') {
      p.byte = 300
      expect(p.byte).toBe(44)
    } else {
      expect(() => {
        p.byte = 300
      }).toThrow()
    }
  })

  it('bool coercion coerces on wasm and throws on node', () => {
    const p = Primitives.of(primitiveInit())
    const assignOne = () => {
      p.flag = 1 as unknown as boolean
    }
    if (target === 'wasm') {
      assignOne()
      expect(p.flag).toBe(true)
    } else {
      expect(assignOne).toThrow()
    }

    const assignEmpty = () => {
      p.flag = '' as unknown as boolean
    }
    if (target === 'wasm') {
      assignEmpty()
      expect(p.flag).toBe(false)
    } else {
      expect(assignEmpty).toThrow()
    }
  })

  it('struct field assignment from a wrong instance is stricter on wasm', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    const assign = () => {
      line.start = Primitives.of(primitiveInit()) as unknown as Point
    }
    if (target === 'wasm') {
      expect(assign).toThrow()
    } else {
      expect(assign).not.toThrow()
      expect(line.start).toBeInstanceOf(Point)
    }
  })

  it('by-ref struct params reject literals only on wasm', () => {
    const call = () => nudge({ x: 1, y: 2 } as Point, 1)
    if (target === 'wasm') {
      expect(call).toThrow()
    } else {
      expect(call).not.toThrow()
    }
  })

  it('too few primitive args throw on node and produce NaN on wasm', () => {
    const tooFew = () => add(1, undefined as unknown as number)
    if (target === 'wasm') {
      expect(Number.isNaN(tooFew())).toBe(true)
    } else {
      expect(tooFew).toThrow()
    }
  })

  it('generated object enumerability differs by target', () => {
    const p = new Point(1, 2)
    const spread = { ...p } as Record<string, unknown>
    if (target === 'wasm') {
      expect(spread).toEqual(
        expect.objectContaining({ __wbg_ptr: expect.any(Number) }),
      )
      expect(Object.keys(p)).toEqual(['__wbg_ptr'])
      expect(Object.entries(p)[0]?.[0]).toBe('__wbg_ptr')
    } else {
      expect(spread).toEqual({})
      expect(Object.keys(p)).toEqual([])
      expect(Object.entries(p)).toEqual([])
    }
    expect(Object.keys(spread).some((k) => k.startsWith('__cursed'))).toBe(
      false,
    )
  })

  it('JSON.stringify exposes wasm pointer but empty node object', () => {
    if (target === 'wasm') {
      expect(JSON.stringify(new Point(1, 2))).toMatch(/"__wbg_ptr":/)
      expect(JSON.stringify(Bigs.of({ signed: makeBigInt(1) }))).toMatch(
        /"__wbg_ptr":/,
      )
    } else {
      expect(JSON.stringify(new Point(1, 2))).toBe('{}')
      expect(JSON.stringify(Bigs.of({ signed: 1 as unknown as bigint }))).toBe(
        '{}',
      )
    }
  })

  it('structuredClone shape differs by target', () => {
    if (target === 'wasm') {
      expect(structuredClone(new Point(1, 2))).toEqual(
        expect.objectContaining({ __wbg_ptr: expect.any(Number) }),
      )
    } else {
      expect(structuredClone(new Point(1, 2))).toEqual({})
    }
  })

  it('all-private struct keys differ by target', () => {
    expect(Object.keys(new Opaque(3))).toEqual(
      target === 'wasm' ? ['__wbg_ptr'] : [],
    )
  })
})

describe('warnings', () => {
  it('unsupported_type fires for each bad type class', () => {
    for (const text of [
      'Vec',
      'Generic("T")',
      'slices',
      'fixed-size arrays',
      'tuples',
      'HashMap',
    ]) {
      expect(hasWarn('rust-curse::unsupported_type', text)).toBe(true)
    }
  })

  it('field_setter_unavailable fires', () => {
    expect(
      hasWarn('rust-curse::field_setter_unavailable', 'Wrapper.inner'),
    ).toBe(true)
  })

  it('primitive_mut_ref_ignored fires for f64 and String', () => {
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"inc"')).toBe(true)
    expect(hasWarn('rust-curse::primitive_mut_ref_ignored', '"append"')).toBe(
      true,
    )
  })

  it('skipped_struct_ref fires for field, param, and return', () => {
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Marker')).toBe(true)
    expect(hasWarn('rust-curse::skipped_struct_ref', 'Pair')).toBe(true)
  })

  it('fn_not_available fires', () => {
    expect(hasWarn('rust-curse::fn_not_available', 'take_noclone')).toBe(true)
  })

  it('method_shadows_field fires', () => {
    expect(hasWarn('rust-curse::method_shadows_field', 'Shadow::len')).toBe(
      true,
    )
  })

  it('method_self_by_value_requires_clone fires', () => {
    expect(
      hasWarn(
        'rust-curse::method_self_by_value_requires_clone',
        'NoClone::consume',
      ),
    ).toBe(true)
  })

  it('unsupported_import fires', () => {
    expect(hasWarn('rust-curse::unsupported_import', 'HashMap')).toBe(true)
  })

  it('warning message names the symbol', () => {
    expect(
      hasWarn('rust-curse::field_setter_unavailable', 'Wrapper.inner'),
    ).toBe(true)
  })
})

describe('data enums', () => {
  it('constructs each variant via .of', () => {
    const wasm = BuildOutput.of({ type: 'wasm', wasm_abs_path: 'a' })
    expect(wasm.type).toBe('wasm')
    expect(wasm.wasm_abs_path).toBe('a')
    const node = BuildOutput.of({ type: 'node', node_abs_path: 'b' })
    expect(node.type).toBe('node')
    expect(node.node_abs_path).toBe('b')
    const skipped = BuildOutput.of({ type: 'skipped' })
    expect(skipped.type).toBe('skipped')
  })

  it('constructs unit and empty variants', () => {
    const full = UnitMix.of({ type: 'full', x: 1 })
    expect(full.type).toBe('full')
    expect(full.x).toBe(1)
    const empty = UnitMix.of({ type: 'empty' })
    expect(empty.type).toBe('empty')
    const unit = UnitMix.of({ type: 'unit' })
    expect(unit.type).toBe('unit')
    for (const value of [empty, unit]) {
      expect(Object.keys(value)).toEqual(target === 'wasm' ? ['__wbg_ptr'] : [])
    }
  })

  it('derives variant tags lower-first', () => {
    const value = CamelTags.of({ type: 'httpGet', code: 1 })
    expect(value.type).toBe('httpGet')
  })

  it('tuple variant enum is unsupported', () => {
    expect((essential as Record<string, unknown>).BadTuple).toBeUndefined()
    expect(
      hasWarn('rust-curse::enum_tuple_variant_unsupported', 'tuple variants'),
    ).toBe(true)
  })

  it('uninhabited enum is unsupported', () => {
    expect((essential as Record<string, unknown>).Uninhabited).toBeUndefined()
    expect(hasWarn('rust-curse::enum_uninhabited', 'no variants')).toBe(true)
  })

  it('shared field name with matching types coalesces', () => {
    const a = SharedOk.of({ type: 'a', data: 1 })
    expect(a.data).toBe(1)
    a.data = 2
    expect(a.data).toBe(2)
    const b = SharedOk.of({ type: 'b', data: 3 })
    expect(b.data).toBe(3)
    b.data = 4
    expect(b.data).toBe(4)
    const c = SharedOk.of({ type: 'c' })
    expect((c as unknown as { data: unknown }).data).toBeUndefined()
    expect(() => {
      ;(c as unknown as { data: number }).data = 5
    }).toThrow()
  })

  it('shared field name with mismatched types is unsupported', () => {
    expect((essential as Record<string, unknown>).SharedBad).toBeUndefined()
    expect(
      hasWarn('rust-curse::enum_shared_field_type_mismatch', 'data'),
    ).toBe(true)
  })

  it('variant tag collision is unsupported', () => {
    expect((essential as Record<string, unknown>).TagCollision).toBeUndefined()
    expect(hasWarn('rust-curse::enum_variant_tag_collision', 'uRL')).toBe(true)
  })

  it('skipped enum reference cascades', () => {
    expect((essential as Record<string, unknown>).UsesColor).toBeUndefined()
    expect(hasWarn('rust-curse::skipped_enum_ref', 'Color')).toBe(true)
  })

  it('active variant field getters', () => {
    const wasm = BuildOutput.of({ type: 'wasm', wasm_abs_path: 'a' })
    expect(wasm.wasm_abs_path).toBe('a')
    expect(
      (wasm as unknown as { node_abs_path: unknown }).node_abs_path,
    ).toBeUndefined()
  })

  it('active variant field setters', () => {
    const wasm = BuildOutput.of({ type: 'wasm', wasm_abs_path: 'a' })
    wasm.wasm_abs_path = 'z'
    expect(wasm.wasm_abs_path).toBe('z')
    expect(() => {
      ;(wasm as unknown as { node_abs_path: string }).node_abs_path = 'q'
    }).toThrow()
  })

  it('escapes the discriminator for a field named type', () => {
    const event = Event.of({ $type: 'click', type: 'mouse', x: 1 })
    expect(event.$type).toBe('click')
    expect(event.type).toBe('mouse')
    expect(event.x).toBe(1)
  })

  it('escapes the discriminator for a method named type and keeps the method callable', () => {
    const tag = Element.of({ $type: 'tag', id: 7 })
    expect(tag.$type).toBe('tag')
    expect(tag.type()).toBe(7)
    const plain = Element.of({ $type: 'plain' })
    expect(plain.$type).toBe('plain')
    expect(plain.type()).toBe(0)
  })

  it('constructs an enum with a struct field', () => {
    const fromInstance = Hitbox.of({ type: 'active', mesh: Mesh.of({ x: 1 }) })
    expect(fromInstance.type).toBe('active')
    expect(fromInstance.mesh).toBeInstanceOf(Mesh)
    expect(fromInstance.mesh.x).toBe(1)
    const fromLiteral = Hitbox.of({ type: 'active', mesh: { x: 1 } })
    expect(fromLiteral.mesh).toBeInstanceOf(Mesh)
    expect(fromLiteral.mesh.x).toBe(1)
  })

  it('mutates a projected struct field through the enum', () => {
    const hitbox = Hitbox.of({ type: 'active', mesh: { x: 1 } })
    hitbox.mesh.x = 9
    expect(hitbox.mesh.x).toBe(9)
  })

  it('detaches a struct field on reassignment', () => {
    const hitbox = Hitbox.of({ type: 'active', mesh: { x: 1 } })
    const old = hitbox.mesh
    hitbox.mesh = Mesh.of({ x: 2 })
    old.x = 5
    expect(hitbox.mesh.x).toBe(2)
  })

  it('assigning a struct field from an object literal is target-specific', () => {
    const hitbox = Hitbox.of({ type: 'active', mesh: { x: 1 } })
    const assign = () => {
      hitbox.mesh = { x: 2 } as Mesh
    }
    if (target === 'wasm') {
      expect(assign).toThrow()
    } else {
      expect(assign).not.toThrow()
    }
  })

  it('constructs an enum with an enum field', () => {
    const fromInstance = Outer.of({ type: 'has', inner: Inner.on(1) })
    expect(fromInstance.inner.type).toBe('on')
    expect(fromInstance.inner.level_or_zero()).toBe(1)
    const fromLiteral = Outer.of({
      type: 'has',
      inner: { type: 'on', level: 1 },
    })
    expect(fromLiteral.inner.type).toBe('on')
    expect(fromLiteral.inner.level_or_zero()).toBe(1)
  })

  it('mutates a projected enum field through the enum', () => {
    const outer = Outer.of({ type: 'has', inner: { type: 'on', level: 1 } })
    outer.inner.set_level(5)
    expect(outer.inner.level_or_zero()).toBe(5)
  })

  it('detaches an enum field on reassignment', () => {
    const outer = Outer.of({ type: 'has', inner: { type: 'on', level: 1 } })
    const oldInner = outer.inner
    outer.inner = Inner.of({ type: 'on', level: 9 })
    oldInner.set_level(3)
    expect(outer.inner.level_or_zero()).toBe(9)
  })

  it('replaces the active variant from a &mut self method', () => {
    const inner = Inner.of({ type: 'on', level: 1 })
    expect(inner.level_or_zero()).toBe(1)
    inner.switch_off()
    expect(inner.type).toBe('off')
    expect(inner.level_or_zero()).toBe(0)
  })

  it('replaces the active variant through a free &mut Enum function', () => {
    const inner = Inner.of({ type: 'on', level: 1 })
    force_off(inner)
    expect(inner.type).toBe('off')
    expect(inner.level_or_zero()).toBe(0)
  })

  it('exposes inherent functions on enums', () => {
    const on = Inner.on(2)
    expect(on.level_or_zero()).toBe(2)
    on.set_level(7)
    expect(on.level_or_zero()).toBe(7)
    on.switch_off()
    expect(on.type).toBe('off')
  })

  it('validates .of discriminators', () => {
    expect(Inner.of({ type: 'on', level: 1 }).type).toBe('on')
    expect(() => Inner.of({} as Inner.Init)).toThrow()
    expect(() =>
      Inner.of({ type: 'nope' } as unknown as Inner.Init),
    ).toThrow()
    expect(() =>
      Inner.of({ type: 'on' } as unknown as Inner.Init),
    ).toThrow()
    const extra = Inner.of({ type: 'on', level: 1, z: 9 } as Inner.Init)
    expect(extra.level_or_zero()).toBe(1)
  })

  it('keeps a user-defined of and escapes the generated factory', () => {
    expect(EnumWithOf.of(1)).toBe(2)
    const built = EnumWithOf.of$({ type: 'a', v: 1 })
    expect(built.type).toBe('a')
    expect(built.v).toBe(1)
  })

  it('constructs through a new constructor', () => {
    const value = new EnumWithNew(3)
    expect(value.type).toBe('active')
    expect(value.level).toBe(3)
  })

  it('pins generated object behavior', () => {
    const value = BuildOutput.of({ type: 'wasm', wasm_abs_path: 'a' })
    expect(value).toBeInstanceOf(BuildOutput)
    const spread = { ...value } as Record<string, unknown>
    if (target === 'wasm') {
      expect(Object.keys(value)).toEqual(['__wbg_ptr'])
      expect(spread).toEqual(
        expect.objectContaining({ __wbg_ptr: expect.any(Number) }),
      )
      expect(JSON.stringify(value)).toMatch(/"__wbg_ptr":/)
      expect(structuredClone(value)).toEqual(
        expect.objectContaining({ __wbg_ptr: expect.any(Number) }),
      )
    } else {
      expect(Object.keys(value)).toEqual([])
      expect(spread).toEqual({})
      expect(JSON.stringify(value)).toBe('{}')
      expect(structuredClone(value)).toEqual({})
    }
    expect(Object.keys(spread).includes('type')).toBe(false)
  })

  it('pins inactive field behavior for each field kind', () => {
    const wasm = BuildOutput.of({ type: 'wasm', wasm_abs_path: 'a' })
    expect(
      (wasm as unknown as { node_abs_path: unknown }).node_abs_path,
    ).toBeUndefined()
    expect(() => {
      ;(wasm as unknown as { node_abs_path: string }).node_abs_path = 'q'
    }).toThrow()

    const disabled = Hitbox.of({ type: 'disabled' })
    expect(
      (disabled as unknown as { mesh: unknown }).mesh,
    ).toBeUndefined()
    expect(() => {
      ;(disabled as unknown as { mesh: Mesh }).mesh = Mesh.of({ x: 1 })
    }).toThrow()

    const none = Outer.of({ type: 'none' })
    expect((none as unknown as { inner: unknown }).inner).toBeUndefined()
    expect(() => {
      ;(none as unknown as { inner: Inner }).inner = Inner.on(1)
    }).toThrow()
  })

  it('drops the non-Clone enum surface', () => {
    expect(typeof NonCloneState).toBe('function')
    expect(new NonCloneState(1).value()).toBe(1)
    expect('of' in NonCloneState).toBe(false)
    expect((essential as Record<string, unknown>).take_state).toBeUndefined()
    expect(hasWarn('rust-curse::fn_not_available', 'take_state')).toBe(true)
    expect('into_value' in NonCloneState.prototype).toBe(false)
    expect(
      hasWarn(
        'rust-curse::method_self_by_value_requires_clone',
        'NonCloneState::into_value',
      ),
    ).toBe(true)
  })

  it('passing a Clone enum by reference does not clone', () => {
    const cc = CloneCounted.new(1)
    reset_enum_clones()
    expect(read_cc(cc)).toBe(1)
    expect(enum_clone_count()).toBe(0)
  })

  it('passing a Clone enum by mutable reference does not clone', () => {
    const cc = CloneCounted.new(1)
    reset_enum_clones()
    bump_cc(cc, 1)
    expect(enum_clone_count()).toBe(0)
    expect(cc.value()).toBe(2)
  })

  it('returning a Clone enum does not clone', () => {
    reset_enum_clones()
    const out = make_cc(5)
    expect(enum_clone_count()).toBe(0)
    expect(out.value()).toBe(5)
  })

  it('passing a Clone enum by value clones', () => {
    const cc = CloneCounted.new(3)
    reset_enum_clones()
    expect(take_cc(cc)).toBe(3)
    expect(enum_clone_count()).toBe(1)
    expect(cc.value()).toBe(3)
  })

  it('passes a Clone enum init literal by value', () => {
    expect(use_inner({ type: 'on', level: 1 })).toBe(1)
  })

  it('.of requires Clone', () => {
    expect('of' in Inner).toBe(true)
    expect('of' in NonCloneState).toBe(false)
  })
})

describe('edge cases', () => {
  it('named empty struct builds through .of({})', () => {
    expect(Empty.of({})).toBeInstanceOf(Empty)
  })

  it('single-field struct builds and round-trips', () => {
    const one = One.of({ v: 1 })
    one.v = 2
    expect(one.v).toBe(2)
  })

  it('pub + private field struct has getter for pub field and no .of', () => {
    const m = new Mixed(1, 2)
    expect(m.a).toBe(1)
    expect(m.sum()).toBe(3)
    expect('of' in Mixed).toBe(false)
  })

  it('all-private-fields struct is usable via methods only', () => {
    const o = new Opaque(3)
    expect(o.value()).toBe(3)
    expect('of' in Opaque).toBe(false)
  })

  it('doc comments propagate to JSDoc', async () => {
    expect(dtsText).toContain('/** A point in 2D space. */')
    expect(dtsText).toContain('/** Horizontal position. */')
  })

  it('deprecated functions are exported without JSDoc deprecation today', async () => {
    expect(old()).toBe(0)
    expect(dtsText).toContain('export function old(): number;')
    expect(dtsText).not.toContain('@deprecated')
  })

  it('fn and struct sharing a name are both available by JS case', () => {
    expect(point()).toBe(12)
    expect(new Point(1, 2).x).toBe(1)
  })

  it('unicode strings round-trip', () => {
    expect(shout('hé 😀')).toBe('HÉ 😀')
  })
})
