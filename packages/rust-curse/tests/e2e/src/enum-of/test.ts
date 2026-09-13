import { expect, it } from 'vitest'
import { Message, Point, Progress, Request, Shape, Status } from './test.rs'

it('dispatches unit, record, and numeric tuple enum inputs', () => {
  expect(Shape.of({ type: 'unit' }).type).toBe('unit')
  const center = Point.of({ 0: 1, 1: 2 })
  const circle = Shape.of({ type: 'circle', center, radius: 3 })
  expect(circle.type).toBe('circle')
  if (circle.type !== 'circle') throw new Error('expected Shape::Circle')
  expect(circle.radius).toBe(3)
  expect(circle.center[0]).toBe(1)

  const rect = Shape.of({
    type: 'rect',
    0: center,
    1: Point.of({ 0: 10, 1: 20 }),
  })
  expect(rect.type).toBe('rect')
  if (rect.type !== 'rect') throw new Error('expected Shape::Rect')
  expect(rect[0][1]).toBe(2)
  expect(rect[1][0]).toBe(10)
  rect[0][0] = 5
  expect(center[0]).toBe(1)
})

it('uses $type when a source field is named type', () => {
  const message = Message.of({ $type: 'text', type: 'text/plain', body: 'hello' })
  expect(message.$type).toBe('text')
  if (message.$type !== 'text') throw new Error('expected Message::Text')
  expect(message.type).toBe('text/plain')
  expect(message.body).toBe('hello')
  expect(Message.of({ $type: 'empty' }).$type).toBe('empty')
})

it('preserves distinct canonical tags when camel-cased names collide', () => {
  const upper = Request.of({ type: 'HTTPRequest', id: 1 })
  const mixed = Request.of({ type: 'HttpRequest', id: 2 })
  expect(upper.type).toBe('HTTPRequest')
  expect(mixed.type).toBe('HttpRequest')
  expect(upper.id).toBe(1)
  expect(mixed.id).toBe(2)
})

it('keeps static Status.of and constructs variants through $of', () => {
  const failed = Status.of(503)
  expect(failed.type).toBe('failed')
  if (failed.type !== 'failed') throw new Error('expected Status::Failed')
  expect(failed.code).toBe(503)
  expect(Status.$of({ type: 'ready' }).type).toBe('ready')
})

it('constructs exhaustive variants of a non-exhaustive parent enum', () => {
  expect(Progress.of({ type: 'started' }).type).toBe('started')
  expect(Progress.of({ type: 'hidden', label: 7 }).label()).toBe(7)
  const progress = Progress.of({ type: 'percent', value: 50 })
  expect(progress.type).toBe('percent')
  if (progress.type !== 'percent') throw new Error('expected Progress::Percent')
  expect(progress.value).toBe(50)
})
