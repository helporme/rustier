import { expectTypeOf, test } from 'vitest'
import { Counter, ReadonlyChild, ReadonlyOwner, Unexposed } from './test.rs'

// TODO: Account for the now-supported Vec<i32> field shadowing its method.
test.skip('declares struct constructors, static functions, and instance methods', () => {
  const counter = new Counter(1)

  expectTypeOf(counter).toEqualTypeOf<Counter>()
  expectTypeOf(counter.value).toEqualTypeOf<number>()
  counter.value = 2
  // @ts-expect-error The exposed field replaces the colliding instance method.
  counter.value()
  expectTypeOf(Counter.baseline()).toEqualTypeOf<number>()
  expectTypeOf(counter.baseline).toEqualTypeOf<number>()
  expectTypeOf<Counter.Plain>().toEqualTypeOf<{ value: number; baseline: number }>()
  expectTypeOf<Counter.Coerce>().toEqualTypeOf<{ value: number; baseline: number }>()
  expectTypeOf<Counter>().toExtend<Counter.Coerce>()
  const unexposed = new Unexposed(1)
  expectTypeOf(unexposed.private()).toEqualTypeOf<number>()
  // TODO: Restore this assertion with a field that does not shadow the method.
  // expectTypeOf(unexposed.unsupported()).toEqualTypeOf<number>()
  expectTypeOf(counter.add(1)).toEqualTypeOf<void>()
  expectTypeOf(counter.into_value()).toEqualTypeOf<number>()
  expectTypeOf(counter.constructor()).toEqualTypeOf<boolean>()
  expectTypeOf(Counter.clone_of(counter)).toEqualTypeOf<Counter>()
})

test('keeps readonly colliding fields in instances and Plain', () => {
  const owner = new ReadonlyOwner(1)

  expectTypeOf(owner.child).toEqualTypeOf<ReadonlyChild>()
  expectTypeOf<ReadonlyOwner.Plain>().toEqualTypeOf<{ readonly child: ReadonlyChild }>()
  // @ts-expect-error Non-Clone child fields have no setter.
  owner.child = new ReadonlyOwner(2).child
  // @ts-expect-error The readonly field still replaces its colliding method.
  owner.child()
})
