import { expect, it } from 'vitest'
import { Circle, Drawing, Opaque, Pair, Point, Vault, circle_center, opaque_value, shift_circle } from './test.rs'

it('coerces Circle with nested Point at a free function boundary', () => {
  const center = circle_center({ center: { x: 3, y: 4 }, radius: 5 })
  expect(center).toBeInstanceOf(Point)
  expect(center.x).toBe(3)
  expect(center.y).toBe(4)
})

it('coerces Point arguments to constructors, static methods and instance methods', () => {
  const circle = new Circle({ x: 1, y: 2 }, 10)
  expect(circle.center.x).toBe(1)
  expect(Circle.centered({ x: 3, y: 4 }).center.y).toBe(4)
  const result = circle.replace_center({ x: 5, y: 6 })
  expect(result).toBeInstanceOf(Point)
  expect(result.x).toBe(5)
  expect(circle.center.y).toBe(6)
})

it('retains observable mutation for existing mutable arguments and Clone ownership for owned arguments', () => {
  const circle = Circle.of({ center: { x: 1, y: 2 }, radius: 10 })
  const point = Point.of({ x: 3, y: 4 })
  expect(circle.shift_point(point)).toBe(13)
  expect(point.x).toBe(13)
  expect(shift_circle(circle)).toBe(2)
  expect(circle.center.x).toBe(2)
  expect(circle.shift_point({ x: 20, y: 30 })).toBe(30)
  expect(shift_circle({ center: { x: 40, y: 50 }, radius: 1 })).toBe(41)
  const clonedCenter = circle_center(circle)
  clonedCenter.x = 99
  expect(circle.center.x).toBe(2)
})

it('coerces named and tuple struct setters without widening getter values', () => {
  const circle = Circle.of({ center: { x: 1, y: 2 }, radius: 3 })
  circle.center = { x: 4, y: 5 }
  expect(circle.center).toBeInstanceOf(Point)
  expect(circle.center.x).toBe(4)
  const pair = Pair.of({ 0: { x: 1, y: 2 }, 1: { type: 'Clear' } })
  pair[0] = { x: 6, y: 7 }
  pair[1] = { type: 'Rgb', red: 8 }
  expect(pair[0]).toBeInstanceOf(Point)
  expect(pair[0].y).toBe(7)
  expect(pair[1].type).toBe('Rgb')
  if (pair[1].type !== 'Rgb') throw new Error('expected Color::Rgb')
  expect(pair[1].red).toBe(8)
})

it('coerces struct and enum fields on named and tuple enum variants', () => {
  const named = Drawing.of({ type: 'Named', point: { x: 1, y: 2 }, color: { type: 'Clear' } })
  if (named.type !== 'Named') throw new Error('expected Drawing::Named')
  named.point = { x: 3, y: 4 }
  named.color = { type: 'Rgb', red: 5 }
  expect(named.point.x).toBe(3)
  expect(named.color.type).toBe('Rgb')
  const tuple = Drawing.of({ type: 'Tuple', 0: { x: 1, y: 2 }, 1: { type: 'Clear' } })
  if (tuple.type !== 'Tuple') throw new Error('expected Drawing::Tuple')
  tuple[0] = { x: 6, y: 7 }
  tuple[1] = { type: 'Rgb', red: 8 }
  expect(tuple[0].y).toBe(7)
  expect(tuple[1].type).toBe('Rgb')
})

it('accepts opaque Clone wrappers in inputs without requiring the nested class to be coercible', () => {
  const opaque = new Opaque(42)
  expect(opaque_value(opaque)).toBe(42)
  const vault = Vault.of({ value: opaque })
  vault.value = new Opaque(7)
  expect(opaque_value(vault.value)).toBe(7)
  expect(opaque_value(opaque)).toBe(42)
})
