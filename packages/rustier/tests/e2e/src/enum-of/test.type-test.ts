import { expectTypeOf, test } from 'vitest'
import { Message, Point, Progress, Request, Shape, Status } from './test.rs'

test('declares coerce inputs as instances or discriminated unions with nested inputs', () => {
  expectTypeOf<Shape.Coerce>().toEqualTypeOf<
    | Shape
    | { readonly type: 'Unit' }
    | { readonly type: 'Circle'; center: Point.Coerce; radius: number }
    | { readonly type: 'Rect'; 0: Point.Coerce; 1: Point.Coerce }
  >()
  expectTypeOf(Shape.of({ type: 'Unit' })).toEqualTypeOf<Shape>()
  // @ts-expect-error Coerce is a namespace type, not a runtime property.
  Shape.Coerce
  expectTypeOf<Message.Coerce>().toEqualTypeOf<
    | Message
    | { readonly $type: 'Text'; type: string; body: string }
    | { readonly $type: 'Empty' }
  >()
  expectTypeOf<Request.Coerce>().toEqualTypeOf<
    | Request
    | { readonly type: 'HTTPRequest'; id: number }
    | { readonly type: 'HttpRequest'; id: number }
  >()
  expectTypeOf(Status.$of({ type: 'Ready' })).toEqualTypeOf<Status>()
  expectTypeOf<Status.Coerce>().toEqualTypeOf<
    | Status
    | { readonly type: 'Ready' }
    | { readonly type: 'Failed'; code: number }
  >()
})

test('excludes nonconstructible variants and rejects invalid enum inputs', () => {
  expectTypeOf<Progress.Coerce>().toEqualTypeOf<
    | Progress
    | { readonly type: 'Started' }
    | { readonly type: 'Percent'; value: number }
    | { readonly type: 'Hidden'; label: number }
  >()
  // @ts-expect-error Non-exhaustive variants cannot be constructed.
  Progress.of({ type: 'Reserved', value: 1 })
  expectTypeOf(Progress.of(Progress.reserved(1))).toEqualTypeOf<Progress>()
  expectTypeOf(Progress.of({ type: 'Hidden', label: 1 })).toEqualTypeOf<Progress>()
  expectTypeOf<Extract<Progress.Plain, { type: 'Hidden' }>>().toEqualTypeOf<{
    readonly type: 'Hidden'
    label: number
  }>()
  // @ts-expect-error A discriminator is required even for a unit variant.
  Shape.of({})
  // @ts-expect-error Unknown variant.
  Shape.of({ type: 'triangle' })
  expectTypeOf(Shape.of({ type: 'Circle', center: { 0: 1, 1: 2 }, radius: 3 })).toEqualTypeOf<Shape>()
  // @ts-expect-error Source field type is not the discriminator.
  Message.of({ type: 'Empty' })
  // @ts-expect-error Variant tags retain their original Rust spelling.
  Request.of({ type: 'http_request', id: 1 })
})
