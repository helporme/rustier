import { describe, expectTypeOf, it } from 'vitest'
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
  nudge,
  origin,
  shift_copy,
  shout,
} from './essential.rs'

describe('Point', () => {
  it('Init is a plain object of the public fields', () => {
    expectTypeOf<Point.Init>().toEqualTypeOf<{ x: number; y: number }>()
  })

  it('constructor takes the field primitives', () => {
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

  it('methods carry their Rust signatures', () => {
    expectTypeOf(Point.prototype.length).returns.toBeNumber()
    expectTypeOf(Point.prototype.scale).parameter(0).toBeNumber()
    expectTypeOf(Point.prototype.scale).returns.toBeVoid()
  })
})

describe('Line', () => {
  it('Init allows a class instance or a nested Init per struct field', () => {
    expectTypeOf<Line.Init>().toEqualTypeOf<{
      start: Point | Point.Init
      end: Point | Point.Init
    }>()
  })

  it('struct fields are typed as the child class', () => {
    expectTypeOf<Line['start']>().toEqualTypeOf<Point>()
    expectTypeOf<Line['end']>().toEqualTypeOf<Point>()
  })

  it('from() accepts class or Init for each param (coercion)', () => {
    expectTypeOf(Line.from).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.from).parameter(1).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.from).returns.toEqualTypeOf<Line>()
  })

  it('exposes the generated factory escaped to of$', () => {
    expectTypeOf(Line.of$).parameter(0).toEqualTypeOf<Line.Init>()
    expectTypeOf(Line.of$).returns.toEqualTypeOf<Line>()
  })

  it('keeps the user-defined of() with its own signature', () => {
    expectTypeOf(Line.of).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(Line.of).returns.toEqualTypeOf<Line>()
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
  it('Init carries every primitive field as its JS type', () => {
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
})

describe('`new` collisions', () => {
  it('keeps a `&self` new as an instance method returning the class', () => {
    expectTypeOf(SelfNew.prototype.new).returns.toEqualTypeOf<SelfNew>()
    expectTypeOf(MutSelfNew.prototype.new).returns.toEqualTypeOf<MutSelfNew>()
  })

  it('keeps a non-Self new as a static method with its own return', () => {
    expectTypeOf(StaticNew.new).parameter(0).toBeNumber()
    expectTypeOf(StaticNew.new).returns.toBeNumber()
  })
})

describe('Wrapper', () => {
  it('exposes a non-clonable struct field as readonly', () => {
    const w = new Wrapper(0)
    // @ts-expect-error `inner` has no setter (NoClone is not Clone)
    w.inner = w.inner
  })
})

describe('functions', () => {
  it('carry their Rust signatures', () => {
    expectTypeOf(add).parameters.toEqualTypeOf<[number, number]>()
    expectTypeOf(add).returns.toBeNumber()
    expectTypeOf(shout).parameter(0).toBeString()
    expectTypeOf(shout).returns.toBeString()
    expectTypeOf(origin).returns.toEqualTypeOf<Point>()
  })

  it('type a by-ref struct param as the class only (no coercion)', () => {
    expectTypeOf(nudge).parameter(0).toEqualTypeOf<Point>()
  })

  it('type a by-value struct param as coercible', () => {
    expectTypeOf(shift_copy).parameter(0).toEqualTypeOf<Point | Point.Init>()
    expectTypeOf(shift_copy).returns.toEqualTypeOf<Point>()
  })
})

describe('negative contracts', () => {
  it('rejects missing or wrongly-typed fields', () => {
    // @ts-expect-error y is required
    Point.of({ x: 1 })
    // @ts-expect-error fields must be numbers
    Point.of({ x: 1, y: 'two' })
    // @ts-expect-error constructor args must be numbers
    new Point('a', 'b')
  })

  it('forbids constructing a struct that has no `new`', () => {
    // @ts-expect-error Line has a private (generated) constructor
    new Line(new Point(0, 0), new Point(0, 0))
    // @ts-expect-error SelfNew's `new` is an instance method, not the ctor
    new SelfNew({ v: 1 })
  })
})
