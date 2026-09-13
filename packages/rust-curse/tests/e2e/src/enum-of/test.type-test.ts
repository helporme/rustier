import { expectTypeOf, test } from 'vitest'
import { Message, Point, Progress, Request, Shape, Status } from './test.rs'

test('declares coerce inputs as discriminated unions with explicit class fields', () => {
  expectTypeOf<Shape.Coerce>().toEqualTypeOf<
    | { readonly type: 'unit' }
    | { readonly type: 'circle'; center: Point; radius: number }
    | { readonly type: 'rect'; 0: Point; 1: Point }
  >()
  expectTypeOf(Shape.of({ type: 'unit' })).toEqualTypeOf<Shape>()
  // @ts-expect-error Coerce is a namespace type, not a runtime property.
  Shape.Coerce
  expectTypeOf<Message.Coerce>().toEqualTypeOf<
    | { readonly $type: 'text'; type: string; body: string }
    | { readonly $type: 'empty' }
  >()
  expectTypeOf<Request.Coerce>().toEqualTypeOf<
    | { readonly type: 'HTTPRequest'; id: number }
    | { readonly type: 'HttpRequest'; id: number }
  >()
  expectTypeOf(Status.$of({ type: 'ready' })).toEqualTypeOf<Status>()
  expectTypeOf<Status.Coerce>().toEqualTypeOf<
    | { readonly type: 'ready' }
    | { readonly type: 'failed'; code: number }
  >()
})

test('excludes nonconstructible variants and rejects invalid enum inputs', () => {
  expectTypeOf<Progress.Coerce>().toEqualTypeOf<
    | { readonly type: 'started' }
    | { readonly type: 'percent'; value: number }
    | { readonly type: 'hidden'; label: number }
  >()
  // @ts-expect-error Non-exhaustive variants cannot be constructed.
  Progress.of({ type: 'reserved', value: 1 })
  expectTypeOf(Progress.of({ type: 'hidden', label: 1 })).toEqualTypeOf<Progress>()
  // @ts-expect-error A discriminator is required even for a unit variant.
  Shape.of({})
  // @ts-expect-error Unknown variant.
  Shape.of({ type: 'triangle' })
  // @ts-expect-error Nested enum fields require wrappers.
  Shape.of({ type: 'circle', center: { 0: 1, 1: 2 }, radius: 3 })
  // @ts-expect-error Source field type is not the discriminator.
  Message.of({ type: 'empty' })
  // @ts-expect-error Colliding canonical tags retain their original case.
  Request.of({ type: 'http_request', id: 1 })
})
