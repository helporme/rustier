import { expectTypeOf, test } from 'vitest'
import { Counter } from './test.rs'

test('declares struct associated functions as static and instance methods', () => {
  const counter = Counter.new(1)

  expectTypeOf(counter).toEqualTypeOf<Counter>()
  expectTypeOf(counter.value()).toEqualTypeOf<number>()
  expectTypeOf(counter.add(1)).toEqualTypeOf<void>()
  expectTypeOf(counter.into_value()).toEqualTypeOf<number>()
  expectTypeOf(counter.constructor()).toEqualTypeOf<boolean>()
  expectTypeOf(Counter.clone_of(counter)).toEqualTypeOf<Counter>()
})
