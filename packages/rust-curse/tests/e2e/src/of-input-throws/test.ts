import { expect, it } from 'vitest'
import { Label, Message, Mixed, Point, Shape, point_x } from './test.rs'

it('rejects incomplete nested Point inputs', () => {
  expect(() => Label.of({ position: { 0: 1 } } as any)).toThrow()
  expect(() => Shape.of({ type: 'circle', center: { 0: 1 } } as any)).toThrow()
})

it('rejects invalid scalar fields in an ordinary Point argument', () => {
  expect(() => point_x({ 0: '1', 1: 2 } as any)).toThrow()
})

it.each(['triangle', 'reserved', undefined])('rejects unavailable Shape tag %s', type => {
  expect(() => Shape.of({ type, value: 1 } as any)).toThrow(
    /Unknown or nonconstructible Shape discriminator/,
  )
})

it('reads the collision-safe $type discriminator rather than the type field', () => {
  expect(() => Message.of({ type: 'empty' } as any)).toThrow(
    /Unknown or nonconstructible Message discriminator/,
  )
})

it('rejects missing required struct and enum fields with valid enum tags', () => {
  expect(() => Label.of({} as any)).toThrow()
  expect(() => Point.of({ 0: 1 } as any)).toThrow()
  expect(() => Shape.of({ type: 'circle' } as any)).toThrow()
  expect(() => Message.of({ $type: 'text' } as any)).toThrow()
})

it.each(['position', 'count', 'enabled', 'text'] as const)('rejects missing Mixed field %s', field => {
  const position = Point.of({ 0: 10, 1: 20 })
  const input: Record<string, unknown> = { position, count: 1, enabled: true, text: 'valid' }
  delete input[field]

  expect(() => Mixed.of(input as any)).toThrow()
  expect(point_x(position)).toBe(10)
  expect(position[1]).toBe(20)
})

it.each([
  ['count', '1'],
  ['enabled', 1],
  ['text', false],
  ['count', 1.5],
  ['count', -129],
  ['count', 128],
] as const)('rejects invalid %s input %s without consuming the nested wrapper', (field, value) => {
  const position = Point.of({ 0: 10, 1: 20 })
  const input = { position, count: 1, enabled: true, text: 'valid' }

  // Position is the first Rust field: later scalar failures must clean up its clone.
  expect(() => Mixed.of({ ...input, [field]: value } as any)).toThrow()
  expect(point_x(position)).toBe(10)
  expect(position[1]).toBe(20)

  const control = Mixed.of(input)
  expect(control.position[0]).toBe(10)
  expect(control.count).toBe(1)
  expect(control.enabled).toBe(true)
  expect(control.text).toBe('valid')
})

it.each([-128, 0, 127])('accepts valid i8 boundary/control value %s', count => {
  const position = Point.of({ 0: 10, 1: 20 })
  const mixed = Mixed.of({ position, count, enabled: false, text: '' })

  expect(mixed.count).toBe(count)
  expect(mixed.enabled).toBe(false)
  expect(mixed.text).toBe('')
  expect(point_x(mixed.position)).toBe(10)
  expect(point_x(position)).toBe(10)
})

it('rejects wrappers lacking required Point fields in struct and enum inputs', () => {
  const wrong = Message.of({ $type: 'empty' })
  expect(() => Label.of({ position: wrong } as any)).toThrow()
  expect(() => Shape.of({ type: 'circle', center: wrong } as any)).toThrow()
  expect(wrong.$type).toBe('empty')

  const position = Point.of({ 0: 10, 1: 20 })
  expect(point_x(Label.of({ position }).position)).toBe(10)
  const circle = Shape.of({ type: 'circle', center: position })
  expect(circle.type).toBe('circle')
  if (circle.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(point_x(circle.center)).toBe(10)
  expect(point_x(position)).toBe(10)
})
