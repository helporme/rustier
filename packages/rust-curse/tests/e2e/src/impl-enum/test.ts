import { expect, it } from 'vitest'
import { Phase } from './test.rs'

it('calls enum static and shared associated functions', () => {
  const waiting = Phase.waiting()
  const running = Phase.running(4)

  expect(waiting.type).toBe('waiting')
  expect(waiting.is_complete()).toBe(false)
  expect(running.type).toBe('running')
  expect(running.step()).toBe(4)
  expect(waiting.same_phase(running)).toBe(false)
})

it('mutates an enum through an exclusive self receiver', () => {
  const phase = Phase.running(3)

  phase.finish()

  expect(phase.type).toBe('complete')
  expect(phase.is_complete()).toBe(true)
  expect(phase.step()).toBe(-1)
})

it('clones an owned enum receiver and leaves the JS wrapper valid', () => {
  const phase = Phase.running(8)

  expect(phase.into_step()).toBe(8)
  expect(phase.type).toBe('running')
  expect(phase.step()).toBe(8)
})

it('passes and returns enum Self values', () => {
  const original = Phase.running(5)
  const cloned = Phase.clone_of(original)

  expect(cloned.type).toBe('running')
  expect(cloned.step()).toBe(5)
  expect(original.same_phase(cloned)).toBe(true)
})

it('lets an enum method shadow variant fields with the same name', () => {
  const phase = Phase.running(11)
  const runtimePhase = phase as unknown as Record<string, unknown>

  expect(phase.step()).toBe(11)
  expect(typeof runtimePhase.step).toBe('function')
})
