import { expectTypeOf, test } from 'vitest'
import {
  make_receiver_new,
  NamedReturn,
  NoArgs,
  Other,
  ReceiverNew,
  WrongReturn,
} from './test.rs'

test('declares eligible new functions as constructors and static methods', () => {
  expectTypeOf(new NoArgs()).toEqualTypeOf<NoArgs>()
  expectTypeOf(new NamedReturn(42)).toEqualTypeOf<NamedReturn>()
  expectTypeOf(NoArgs.new()).toEqualTypeOf<NoArgs>()
  expectTypeOf(NamedReturn.new(42)).toEqualTypeOf<NamedReturn>()
})

test('retains ineligible new functions as methods', () => {
  const receiver = make_receiver_new(5)

  expectTypeOf(receiver).toEqualTypeOf<ReceiverNew>()
  expectTypeOf(receiver.new()).toEqualTypeOf<ReceiverNew>()
  expectTypeOf(WrongReturn.new(9)).toEqualTypeOf<Other>()
})
