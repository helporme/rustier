import { expect, it } from 'vitest'
import { rustCurseDiags } from '$build/diags'
import { Phase, UploadState } from './test.rs'

it('calls Phase::new as a static function rather than a constructor', () => {
  const phase = Phase.new()

  expect(phase.type).toBe('waiting')
  expect(phase.is_complete()).toBe(false)
})

it('passes arguments to enum new and returns the selected variant fields', () => {
  const state = UploadState.new(25)

  expect(state.type).toBe('active')
  if (state.type !== 'active') {
    throw new Error('UploadState.new must return the active variant')
  }
  expect(state.progress).toBe(25)
})

it('calls enum static and shared associated functions', () => {
  const waiting = Phase.waiting()
  const running = Phase.running(4)

  expect(waiting.type).toBe('waiting')
  expect(waiting.is_complete()).toBe(false)
  expect(running.type).toBe('running')
  if (running.type !== 'running') throw new Error('expected Phase::Running')
  expect(running.step).toBe(4)
  running.step = 6
  expect(running.into_step()).toBe(6)
  expect(waiting.same_phase(running)).toBe(false)
})

it('mutates an enum through an exclusive self receiver', () => {
  const phase = Phase.running(3)

  phase.finish()

  expect(phase.type).toBe('complete')
  expect(phase.is_complete()).toBe(true)
  expect(phase.into_step()).toBe(-1)
})

it('clones an owned enum receiver and leaves the JS wrapper valid', () => {
  const phase = Phase.running(8)

  expect(phase.into_step()).toBe(8)
  expect(phase.type).toBe('running')
  if (phase.type !== 'running') throw new Error('expected Phase::Running')
  expect(phase.step).toBe(8)
})

it('passes and returns enum Self values', () => {
  const original = Phase.running(5)
  const cloned = Phase.clone_of(original)

  expect(cloned.type).toBe('running')
  if (cloned.type !== 'running') throw new Error('expected Phase::Running')
  expect(cloned.step).toBe(5)
  expect(original.same_phase(cloned)).toBe(true)
})

it('omits a colliding enum method globally, even on variants without the field', () => {
  const phase = Phase.running(11)

  if (phase.type !== 'running') throw new Error('expected Phase::Running')
  expect(phase.step).toBe(11)
  // Inactive fields still have accessors that reject reads; no callable method remains.
  const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(Phase.waiting()), 'step')
  expect(typeof descriptor?.get).toBe('function')
  expect(descriptor?.value).toBeUndefined()
  phase.finish()
  expect(phase.into_step()).toBe(-1)
  expect(rustCurseDiags.filter(diag =>
    diag.code === 'rust_curse::method_shadowed_by_field'
    && diag.message.includes('e2e::impl_enum::Phase::step'),
  )).toHaveLength(1)
})
