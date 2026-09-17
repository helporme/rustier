import { expectTypeOf, test } from 'vitest'
import { Message, Point, Progress, Request, Shape, Status } from './test.rs'

test('declares coerce inputs as instances or discriminated unions with nested inputs', () => {
  expectTypeOf<Shape.Coerce>().toEqualTypeOf<
    | Shape
    | { readonly type: 'unit' }
    | { readonly type: 'circle'; center: Point.Coerce; radius: number }
    | { readonly type: 'rect'; 0: Point.Coerce; 1: Point.Coerce }
  >()
  expectTypeOf(Shape.of({ type: 'unit' })).toEqualTypeOf<Shape>()
  // @ts-expect-error Coerce is a namespace type, not a runtime property.
  Shape.Coerce
  expectTypeOf<Message.Coerce>().toEqualTypeOf<
    | Message
    | { readonly $type: 'text'; type: string; body: string }
    | { readonly $type: 'empty' }
  >()
  expectTypeOf<Request.Coerce>().toEqualTypeOf<
    | Request
    | { readonly type: 'HTTPRequest'; id: number }
    | { readonly type: 'HttpRequest'; id: number }
  >()
  expectTypeOf(Status.$of({ type: 'ready' })).toEqualTypeOf<Status>()
  expectTypeOf<Status.Coerce>().toEqualTypeOf<
    | Status
    | { readonly type: 'ready' }
    | { readonly type: 'failed'; code: number }
  >()
})

test('excludes nonconstructible variants and rejects invalid enum inputs', () => {
  expectTypeOf<Progress.Coerce>().toEqualTypeOf<
    | Progress
    | { readonly type: 'started' }
    | { readonly type: 'percent'; value: number }
    | { readonly type: 'hidden'; label: number }
  >()
  // @ts-expect-error Non-exhaustive variants cannot be constructed.
  Progress.of({ type: 'reserved', value: 1 })
  expectTypeOf(Progress.of(Progress.reserved(1))).toEqualTypeOf<Progress>()
  expectTypeOf(Progress.of({ type: 'hidden', label: 1 })).toEqualTypeOf<Progress>()
  expectTypeOf<Extract<Progress.Plain, { type: 'hidden' }>>().toEqualTypeOf<{
    readonly type: 'hidden'
    label: number
  }>()
  // @ts-expect-error A discriminator is required even for a unit variant.
  Shape.of({})
  // @ts-expect-error Unknown variant.
  Shape.of({ type: 'triangle' })
  expectTypeOf(Shape.of({ type: 'circle', center: { 0: 1, 1: 2 }, radius: 3 })).toEqualTypeOf<Shape>()
  // @ts-expect-error Source field type is not the discriminator.
  Message.of({ type: 'empty' })
  // @ts-expect-error Colliding canonical tags retain their original case.
  Request.of({ type: 'http_request', id: 1 })
})
