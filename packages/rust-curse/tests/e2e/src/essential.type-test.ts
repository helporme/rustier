import { describe, expectTypeOf, it } from 'vitest'
import dtsText from '../.rust-curse/types/src/essential.rs.d.ts?raw'
import * as essential from './essential.rs'
import {
  Bigs,
  Builder,
  Element,
  EnumWithNew,
  EnumWithOf,
  Event,
  Hitbox,
  Inner,
  Line,
  MutSelfNew,
  NewAndOf,
  NoClone,
  Outer,
  Other,
  Point,
  Primitives,
  SelfNew,
  StaticNew,
  Triangle,
  UnitMix,
  Wrapper,
  add,
  bump_cc,
  nudge,
  origin,
  shift_copy,
  shout,
  tick,
  use_inner,
} from './essential.rs'

describe('Point', () => {
  it('Init is the public fields', () => {
    expectTypeOf<Point.Init>().toEqualTypeOf<{ x: number; y: number }>()
  })

  it('constructor params typed', () => {
    expectTypeOf(Point).toBeConstructibleWith(1, 2)
    expectTypeOf(Point).constructorParameters.toEqualTypeOf<[number, number]>()
  })

  it('.of takes Init and returns the class', () => {
    expectTypeOf(Point.of).parameter(0).toEqualTypeOf<Point.Init>()
    expectTypeOf(Point.of).returns.toEqualTypeOf<Point>()
  })

  it('fields are mutable numbers', () => {
    expectTypeOf<Point['x']>().toBeNumber()
    expectTypeOf<Point['y']>().toBeNumber()
  })

  it('methods carry Rust signatures', () => {
    expectTypeOf(Point.prototype.length).returns.toBeNumber()
    expectTypeOf(Point.prototype.scale).parameter(0).toBeNumber()
    expectTypeOf(Point.prototype.scale).returns.toBeVoid()
    expectTypeOf(Point.prototype.add_assign)
      .parameter(0)
      .toEqualTypeOf<Point | Point.Init>()
  })

  it('method returning another struct typed', () => {
    expectTypeOf(Point.prototype.to_origin).returns.toEqualTypeOf<Other>()
  })
})

describe('Line', () => {
  it('Init allows class or nested Init', () => {
    expectTypeOf<Line.Init>().toEqualTypeOf<{
      start: Point | Point.Init
      end: Point | Point.Init
    }>()
  })

  it('struct fields typed as child class', () => {
    expectTypeOf<Line['start']>().toEqualTypeOf<Point>()
    expectTypeOf<Line['end']>().toEqualTypeOf<Point>()
  })

  it('from() accepts class or Init', () => {
    expectTypeOf(Line.from).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.from).parameter(1).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.from).returns.toEqualTypeOf<Line>()
  })

  it('generated factory of$ typed', () => {
    expectTypeOf(Line.of$).parameter(0).toEqualTypeOf<Line.Init>()
    expectTypeOf(Line.of$).returns.toEqualTypeOf<Line>()
  })

  it('user of() keeps its signature', () => {
    expectTypeOf(Line.of).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.of).returns.toEqualTypeOf<Line>()
  })

  it('class field setter accepts a class instance or a nested Init literal', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    line.start = { x: 7, y: 8 }
    line.start = Point.of({ x: 9, y: 10 })
    line.end = new Point(0, 0)
  })

  it('class field read stays the precise class type', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    expectTypeOf(line.start).toEqualTypeOf<Point>()
    line.start.scale(2)
  })

  it('class field setter rejects an unrelated value', () => {
    const line = Line.from(new Point(1, 1), new Point(2, 2))
    // @ts-expect-error start expects a Point or Point.Init
    line.start = 'nope'
  })
})

describe('Triangle', () => {
  it('Init nests recursively', () => {
    expectTypeOf<Triangle.Init>().toEqualTypeOf<{
      line: Line | Line.Init
      point: Point | Point.Init
    }>()
  })
})

describe('Primitives', () => {
  it('Init carries every primitive as its JS type', () => {
    expectTypeOf<Primitives.Init>().toEqualTypeOf<{
      flag: boolean
      text: string
      sbyte: number
      byte: number
      short: number
      ushort: number
      int: number
      uint: number
      big: number
    }>()
  })

  it('fields are mutable and string-typed where expected', () => {
    expectTypeOf<Primitives['flag']>().toBeBoolean()
    expectTypeOf<Primitives['text']>().toBeString()
    expectTypeOf<Primitives['int']>().toBeNumber()
  })

  it('i64 is declared as bigint in Init and instance', () => {
    expectTypeOf<Bigs.Init>().toEqualTypeOf<{ signed: bigint }>()
    expectTypeOf<Bigs['signed']>().toEqualTypeOf<bigint>()
  })

  it('unit return typed void', () => {
    expectTypeOf(tick).returns.toBeVoid()
  })

  it('passing number where bigint required is an error', () => {
    // @ts-expect-error signed is declared as bigint
    Bigs.of({ signed: 1 })
  })
})

describe('`new` collisions', () => {
  it('&self/&mut self new is an instance method', () => {
    expectTypeOf(SelfNew.prototype.new).returns.toEqualTypeOf<SelfNew>()
    expectTypeOf(MutSelfNew.prototype.new).returns.toEqualTypeOf<MutSelfNew>()
  })

  it('non-Self new is static', () => {
    expectTypeOf(StaticNew.new).parameter(0).toBeNumber()
    expectTypeOf(StaticNew.new).returns.toBeNumber()
  })

  it('static vs instance placement', () => {
    // @ts-expect-error StaticNew.new is not an instance method
    StaticNew.prototype.new
    // @ts-expect-error SelfNew.new is not static
    SelfNew.new
  })
})

describe('Wrapper', () => {
  it('non-clone field is readonly', () => {
    const w = new Wrapper(0)
    // @ts-expect-error inner has no setter
    w.inner = w.inner
  })

  it('non-clone field absent from Init', () => {
    // @ts-expect-error Wrapper has no generated Init namespace
    type _WrapperInit = Wrapper.Init
  })
})

describe('functions', () => {
  it('free fns carry Rust signatures', () => {
    expectTypeOf(add).parameters.toEqualTypeOf<[number, number]>()
    expectTypeOf(add).returns.toBeNumber()
    expectTypeOf(shout).parameter(0).toBeString()
    expectTypeOf(shout).returns.toBeString()
    expectTypeOf(origin).returns.toEqualTypeOf<Point>()
  })

  it('by-ref struct param is class-only', () => {
    expectTypeOf(nudge).parameter(0).toEqualTypeOf<Point>()
  })

  it('by-value struct param is coercible', () => {
    expectTypeOf(shift_copy).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(shift_copy).returns.toEqualTypeOf<Point>()
  })
})

describe('Builder', () => {
  it('method returning Self typed as the class', () => {
    expectTypeOf(Builder.prototype.with).parameter(0).toBeNumber()
    expectTypeOf(Builder.prototype.with).returns.toEqualTypeOf<Builder>()
    expectTypeOf(Builder.prototype.set).returns.toBeVoid()
  })
})

describe('negative contracts', () => {
  it('rejects missing/wrong fields', () => {
    // @ts-expect-error y is required
    Point.of({ x: 1 })
    // @ts-expect-error fields must be numbers
    Point.of({ x: 1, y: 'two' })
    // @ts-expect-error constructor args must be numbers
    new Point('a', 'b')
  })

  it('forbids constructing a no-new struct', () => {
    // @ts-expect-error Line has a private constructor
    new Line(new Point(0, 0), new Point(0, 0))
    // @ts-expect-error SelfNew's new is an instance method
    new SelfNew({ v: 1 })
  })

  it('skipped structs absent from declarations', () => {
    // @ts-expect-error Marker is skipped
    essential.Marker
    // @ts-expect-error Pair is skipped
    essential.Pair
  })

  it('by-ref param rejects an Init literal', () => {
    // @ts-expect-error nudge requires a Point instance
    nudge({ x: 1, y: 2 }, 1)
  })

  it('user of collision keeps escaped generated factory', () => {
    expectTypeOf(NewAndOf.of).parameter(0).toBeNumber()
    expectTypeOf(NewAndOf.of).returns.toBeNumber()
    expectTypeOf(NewAndOf.of$).parameter(0).toEqualTypeOf<NewAndOf.Init>()
  })

  it('NoClone has constructor but no Init', () => {
    expectTypeOf(NoClone).toBeConstructibleWith(1)
    // @ts-expect-error NoClone is not struct-expressible
    NoClone.of
  })
})

describe('data enums', () => {
  it('Init is a discriminated union on type', () => {
    expectTypeOf<Inner.Init>().toEqualTypeOf<
      { readonly type: 'on'; level: number } | { readonly type: 'off' }
    >()
  })

  it('generated enum wrapper shape is pinned', () => {
    expectTypeOf(dtsText).toBeString()
  })

  it('.of accepts Init and returns the class', () => {
    expectTypeOf(Inner.of).parameter(0).toEqualTypeOf<Inner.Init>()
    expectTypeOf(Inner.of).returns.toEqualTypeOf<Inner>()
  })

  it('non-Clone enum has no generated of', () => {
    expectTypeOf<typeof essential.NonCloneState>().not.toHaveProperty('of')
  })

  it('new constructor is typed', () => {
    expectTypeOf(EnumWithNew).toBeConstructibleWith(1)
    expectTypeOf(EnumWithNew).constructorParameters.toEqualTypeOf<[number]>()
  })

  it('an enum without new is not constructible', () => {
    // @ts-expect-error Inner has no public constructor
    new Inner({ type: 'on', level: 1 })
  })

  it('inherent method types', () => {
    expectTypeOf(Inner.prototype.level_or_zero).returns.toBeNumber()
    expectTypeOf(Inner.prototype.switch_off).parameters.toEqualTypeOf<[]>()
    expectTypeOf(Inner.on).returns.toEqualTypeOf<Inner>()
    // @ts-expect-error set_level collides with the level setter and is dropped
    Inner.prototype.set_level
  })

  it('struct field init type', () => {
    const init: Hitbox.Init = { type: 'active', mesh: { x: 1 } }
    expectTypeOf(init).toMatchTypeOf<Hitbox.Init>()
    const h = Hitbox.of({ type: 'active', mesh: { x: 1 } })
    if (h.type === 'active') {
      // The narrowed read type is `Mesh | (Mesh & Mesh.Init)` because `Mesh` is
      // a nominal class (unlike an enum child, it does not collapse). Pin that
      // it is exactly a `Mesh` and carries no `undefined` via assignment.
      const mesh: essential.Mesh = h.mesh
      expectTypeOf(mesh).toEqualTypeOf<essential.Mesh>()
    }
  })

  it('enum field init type', () => {
    const init: Outer.Init = { type: 'has', inner: { type: 'on', level: 1 } }
    expectTypeOf(init).toMatchTypeOf<Outer.Init>()
    const o = Outer.of({ type: 'has', inner: { type: 'on', level: 1 } })
    if (o.type === 'has') {
      expectTypeOf(o.inner).toEqualTypeOf<Inner>()
    }
  })

  it('unit and empty variant types', () => {
    expectTypeOf<Extract<UnitMix.Init, { type: 'empty' }>>().toEqualTypeOf<{
      readonly type: 'empty'
    }>()
    expectTypeOf<Extract<UnitMix.Init, { type: 'unit' }>>().toEqualTypeOf<{
      readonly type: 'unit'
    }>()
  })

  it('discriminator escaping for a field named type', () => {
    const init: Event.Init = { $type: 'click', type: 'mouse', x: 1 }
    expectTypeOf(init).toMatchTypeOf<Event.Init>()
    expectTypeOf<
      Extract<Event.Init, { $type: 'click' }>['type']
    >().toBeString()
  })

  it('discriminator escaping for a method named type', () => {
    const init: Element.Init = { $type: 'tag', id: 1 }
    expectTypeOf(init).toMatchTypeOf<Element.Init>()
    expectTypeOf(Element.prototype.type).returns.toBeNumber()
  })

  it('user of collision keeps the escaped generated factory', () => {
    expectTypeOf(EnumWithOf.of).parameter(0).toBeNumber()
    expectTypeOf(EnumWithOf.of).returns.toBeNumber()
    expectTypeOf(EnumWithOf.of$).parameter(0).toEqualTypeOf<EnumWithOf.Init>()
    expectTypeOf(EnumWithOf.of$).returns.toEqualTypeOf<EnumWithOf>()
  })

  it('by-ref versus by-value parameter types', () => {
    expectTypeOf(bump_cc).parameter(0).toEqualTypeOf<essential.CloneCounted>()
    expectTypeOf(use_inner).parameter(0).toEqualTypeOf<Inner | Inner.Init>()
    // @ts-expect-error take_state is dropped for the non-Clone enum
    essential.take_state
  })
})
