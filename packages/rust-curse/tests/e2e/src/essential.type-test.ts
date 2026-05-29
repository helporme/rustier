import { describe, expectTypeOf, it } from 'vitest'
import { Line, Point, Triangle } from './essential.rs'

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
  })
})
