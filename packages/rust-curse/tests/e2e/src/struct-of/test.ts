import { expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import { Counter, Label, Point, Ready, Ticket, point_x } from './test.rs'

it('constructs unit, tuple, and record structs with explicit inputs', () => {
  expect(Ready.of({})).toBeInstanceOf(Ready)

  const position = Point.of({ 0: 10, 1: 20 })
  const label = Label.of({ text: 'origin', position })
  expect(label.text).toBe('origin')
  expect(label.position[0]).toBe(10)
  expect(label.position[1]).toBe(20)

  // Passing a wrapper clones its Rust value, not its JS identity or ownership.
  label.position[0] = 30
  expect(position[0]).toBe(10)
  expect(point_x(position)).toBe(10)
  expect(position[1]).toBe(20)
})

it('coerces exposed fields without shadowing an instance of method', () => {
  const counter = Counter.of({ value: 7 })
  expect(counter.read_value()).toBe(7)
  expect(counter.value).toBe(7)
  expect(counter.of()).toBe(8)
})

it.each([undefined, null, 42, 'input'])('rejects non-object unit input %s', value => {
  expect(() => Ready.of(value as any)).toThrow(TypeError)
})

it('keeps static Ticket.of and exposes the generated constructor as $of', () => {
  expect(Ticket.of(7).number).toBe(107)
  expect(Ticket.$of({ number: 7 }).number).toBe(7)
  const ticket = Ticket.$of({ number: 7 })
  expect(Ticket.$of(Ticket.$of(ticket))).toBe(ticket)
  const renderedDiags = rustCurseDiags.map(diag => diag.rendered).join('\n')
  expect(renderedDiags).toContain('rust_curse::coerce_name_collision')
})

it('returns the same Point instance through repeated coercion', () => {
  const point = Point.of({ 0: 1, 1: 2 })
  expect(Point.of(Point.of(point))).toBe(point)
  expect(Label.of({ text: 'nested', position: { 0: 3, 1: 4 } }).position[0]).toBe(3)
  expect(point_x({ 0: 5, 1: 6 })).toBe(5)
})
