import { expectTypeOf, test } from 'vitest'
import { Circle, Color, Drawing, Opaque, Pair, Point, Vault, circle_center, opaque_value, shift_circle } from './test.rs'
import { Coerce as NamedCoerce, Plain as NamedPlain } from './test.rs'

test('accepts instances whose class names overlap generated namespace types', () => {
  const record = NamedCoerce.of({ value: 42 })
  const variant = NamedPlain.of({ type: 'Unit' })
  expectTypeOf<NamedCoerce>().toExtend<NamedCoerce.Coerce>()
  expectTypeOf<NamedPlain>().toExtend<NamedPlain.Coerce>()
  expectTypeOf(NamedCoerce.of(record)).toEqualTypeOf<NamedCoerce>()
  expectTypeOf(NamedPlain.of(variant)).toEqualTypeOf<NamedPlain>()
})

test('widens every class input boundary while keeping results as exact wrappers', () => {
  expectTypeOf<Parameters<typeof circle_center>>().toEqualTypeOf<[Circle.Coerce]>()
  expectTypeOf<Parameters<typeof shift_circle>>().toEqualTypeOf<[Circle.Coerce]>()
  expectTypeOf<ConstructorParameters<typeof Circle>>().toEqualTypeOf<[Point.Coerce, number]>()
  expectTypeOf<Parameters<typeof Circle.centered>>().toEqualTypeOf<[Point.Coerce]>()
  expectTypeOf<Parameters<Circle['replace_center']>>().toEqualTypeOf<[Point.Coerce]>()
  expectTypeOf<Parameters<Circle['shift_point']>>().toEqualTypeOf<[Point.Coerce]>()
  expectTypeOf(circle_center({ center: { x: 1, y: 2 }, radius: 3 })).toEqualTypeOf<Point>()
  expectTypeOf(Circle.centered({ x: 1, y: 2 })).toEqualTypeOf<Circle>()
  const circle = new Circle({ x: 1, y: 2 }, 3)
  expectTypeOf(circle.replace_center({ x: 4, y: 5 })).toEqualTypeOf<Point>()
  circle.center = { x: 6, y: 7 }
  expectTypeOf(circle.center).toEqualTypeOf<Point>()
  expectTypeOf<Circle['center']>().toEqualTypeOf<Point>()
})

test('accepts setter input objects while named and tuple getter types stay exact', () => {
  const pair = Pair.of({ 0: { x: 1, y: 2 }, 1: { type: 'Clear' } })
  pair[0] = { x: 3, y: 4 }
  pair[1] = { type: 'Rgb', red: 5 }
  expectTypeOf(pair[0]).toEqualTypeOf<Point>()
  expectTypeOf(pair[1]).toEqualTypeOf<Color>()
  const named = Drawing.of({ type: 'Named', point: { x: 1, y: 2 }, color: { type: 'Clear' } })
  if (named.type === 'Named') {
    named.point = { x: 3, y: 4 }
    named.color = { type: 'Rgb', red: 5 }
    expectTypeOf(named.point).toEqualTypeOf<Point>()
    expectTypeOf(named.color).toEqualTypeOf<Color>()
  }
  const tuple = Drawing.of({ type: 'Tuple', 0: { x: 1, y: 2 }, 1: { type: 'Clear' } })
  if (tuple.type === 'Tuple') {
    tuple[0] = { x: 3, y: 4 }
    tuple[1] = { type: 'Rgb', red: 5 }
    expectTypeOf(tuple[0]).toEqualTypeOf<Point>()
    expectTypeOf(tuple[1]).toEqualTypeOf<Color>()
  }
})

test('keeps noncoercible Clone inputs wrapper-only and retains required-field and scalar guards', () => {
  expectTypeOf<Parameters<typeof opaque_value>>().toEqualTypeOf<[Opaque]>()
  expectTypeOf<Vault.Coerce>().toEqualTypeOf<{ value: Opaque }>()
  const vault = Vault.of({ value: new Opaque(42) })
  // @ts-expect-error Private-field classes have no Coerce input.
  opaque_value({ value: 42 })
  // @ts-expect-error A nested noncoercible field still requires a wrapper.
  Vault.of({ value: { value: 42 } })
  // @ts-expect-error Noncoercible setters remain wrapper-only.
  vault.value = { value: 42 }
  // @ts-expect-error Nested Point.y is required.
  circle_center({ center: { x: 1 }, radius: 3 })
  // @ts-expect-error Scalar inputs do not become coercible.
  Circle.centered({ x: '1', y: 2 })
  const circle = new Circle({ x: 1, y: 2 }, 3)
  // @ts-expect-error Setter input fields remain required.
  circle.center = { x: 4 }
  // @ts-expect-error Return values remain branded classes, not input objects.
  const returned: ReturnType<typeof circle_center> = { x: 1, y: 2 }
})
