import { expect, it } from 'vitest'
import { Message, Point, Progress, Request, Shape, Status } from './test.rs'

it('dispatches unit, record, and numeric tuple enum inputs', () => {
  expect(Shape.of({ type: 'Unit' }).type).toBe('Unit')
  const center = Point.of({ 0: 1, 1: 2 })
  const circle = Shape.of({ type: 'Circle', center, radius: 3 })
  expect(circle.type).toBe('Circle')
  if (circle.type !== 'Circle') throw new Error('expected Shape::Circle')
  expect(circle.radius).toBe(3)
  expect(circle.center[0]).toBe(1)

  const rect = Shape.of({
    type: 'Rect',
    0: center,
    1: Point.of({ 0: 10, 1: 20 }),
  })
  expect(rect.type).toBe('Rect')
  if (rect.type !== 'Rect') throw new Error('expected Shape::Rect')
  expect(rect[0][1]).toBe(2)
  expect(rect[1][0]).toBe(10)
  rect[0][0] = 5
  expect(center[0]).toBe(1)
})

it('uses $type when a source field is named type', () => {
  const message = Message.of({ $type: 'Text', type: 'text/plain', body: 'hello' })
  expect(message.$type).toBe('Text')
  if (message.$type !== 'Text') throw new Error('expected Message::Text')
  expect(message.type).toBe('text/plain')
  expect(message.body).toBe('hello')
  expect(Message.of({ $type: 'Empty' }).$type).toBe('Empty')
})

it('preserves exact Rust variant names as discriminant tags', () => {
  const upper = Request.of({ type: 'HTTPRequest', id: 1 })
  const mixed = Request.of({ type: 'HttpRequest', id: 2 })
  expect(upper.type).toBe('HTTPRequest')
  expect(mixed.type).toBe('HttpRequest')
  expect(upper.id).toBe(1)
  expect(mixed.id).toBe(2)
})

it('keeps static Status.of and constructs variants through $of', () => {
  const failed = Status.of(503)
  expect(failed.type).toBe('Failed')
  if (failed.type !== 'Failed') throw new Error('expected Status::Failed')
  expect(failed.code).toBe(503)
  expect(Status.$of({ type: 'Ready' }).type).toBe('Ready')
  expect(Status.$of(Status.$of(failed))).toBe(failed)
})

it('constructs exhaustive variants of a non-exhaustive parent enum', () => {
  expect(Progress.of({ type: 'Started' }).type).toBe('Started')
  const hidden = Progress.of({ type: 'Hidden', label: 7 })
  if (hidden.type !== 'Hidden') throw new Error('expected Progress::Hidden')
  expect(hidden.label).toBe(7)
  const progress = Progress.of({ type: 'Percent', value: 50 })
  expect(progress.type).toBe('Percent')
  if (progress.type !== 'Percent') throw new Error('expected Progress::Percent')
  expect(progress.value).toBe(50)
})

it('preserves enum identity including colliding fields and nonconstructible variants', () => {
  const circle = Shape.of({ type: 'Circle', center: { 0: 1, 1: 2 }, radius: 3 })
  expect(Shape.of(Shape.of(circle))).toBe(circle)
  const hidden = Progress.of({ type: 'Hidden', label: 7 })
  expect(Progress.of(Progress.of(hidden))).toBe(hidden)
  if (hidden.type !== 'Hidden') throw new Error('expected Progress::Hidden')
  expect(hidden.label).toBe(7)
  hidden.label = 8
  expect(hidden.label).toBe(8)
  const reserved = Progress.reserved(42)
  expect(Progress.of(Progress.of(reserved))).toBe(reserved)
  expect(reserved.type).toBe('Reserved')
  if (reserved.type !== 'Reserved') throw new Error('expected Progress::Reserved')
  expect(reserved.value).toBe(42)
})
