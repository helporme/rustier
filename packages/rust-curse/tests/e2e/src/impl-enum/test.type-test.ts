import { expectTypeOf, test } from 'vitest'
import { Phase } from './test.rs'

test('declares enum associated functions on its namespace and instance', () => {
  const phase = Phase.running(1)

  expectTypeOf(phase).toEqualTypeOf<Phase>()
  expectTypeOf(phase.type).toEqualTypeOf<'waiting' | 'running' | 'complete'>()
  expectTypeOf(phase.step()).toEqualTypeOf<number>()
  expectTypeOf(phase.finish()).toEqualTypeOf<void>()
  expectTypeOf(Phase.clone_of(phase)).toEqualTypeOf<Phase>()
})
