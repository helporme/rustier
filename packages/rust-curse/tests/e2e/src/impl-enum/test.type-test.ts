import { expectTypeOf, test } from 'vitest'
import { DownloadState, Phase, UploadState } from './test.rs'

test('declares enum associated functions on its static value and instance', () => {
  const phase = Phase.running(1)

  expectTypeOf(phase).toEqualTypeOf<Phase>()
  expectTypeOf(phase.type).toEqualTypeOf<'waiting' | 'running' | 'complete'>()
  if (phase.type === 'running') {
    expectTypeOf(phase.step).toEqualTypeOf<number>()
    phase.step = 2
    // @ts-expect-error The field replaces the method rather than overloading it.
    phase.step()
  } else {
    // @ts-expect-error A collision on Running removes the method from all variants.
    phase.step
  }
  expectTypeOf(phase.into_step()).toEqualTypeOf<number>()
  expectTypeOf(phase.is_complete()).toEqualTypeOf<boolean>()
  expectTypeOf(phase.finish()).toEqualTypeOf<void>()
  expectTypeOf(Phase.clone_of(phase)).toEqualTypeOf<Phase>()
})

test('declares Phase.new as callable without making Phase constructible', () => {
  expectTypeOf(Phase.new()).toEqualTypeOf<Phase>()

  // @ts-expect-error Rust enum new is a static method, not a JS constructor.
  new Phase()
})

test('exposes Phase.Instance only as a type', () => {
  const instance: Phase.Instance = Phase.new()

  expectTypeOf(instance.into_step()).toEqualTypeOf<number>()
  expectTypeOf<Phase.Instance>().not.toHaveProperty('step')
  // @ts-expect-error Instance describes Rust wrappers; it has no runtime value.
  Phase.Instance
  // @ts-expect-error Instance cannot be used to construct a wrapper.
  new Phase.Instance()
})

test('includes colliding fields in the enum Plain union', () => {
  expectTypeOf<Phase.Plain>().toEqualTypeOf<
    | { readonly type: 'waiting' }
    | { readonly type: 'running'; step: number }
    | { readonly type: 'complete' }
  >()
})

test('narrows branded enum instances by their variant tag', () => {
  const state = UploadState.new(25)

  expectTypeOf(state).toEqualTypeOf<UploadState>()
  expectTypeOf(state.type).toEqualTypeOf<'idle' | 'active'>()
  if (state.type === 'active') {
    expectTypeOf(state.progress).toEqualTypeOf<number>()
    expectTypeOf(state.type).toEqualTypeOf<'active'>()
  } else {
    expectTypeOf(state.type).toEqualTypeOf<'idle'>()
    // @ts-expect-error The idle variant has no progress field.
    state.progress
  }
})

test('accepts variant function inputs without making plain values branded instances', () => {
  const plain: UploadState.Plain = { type: 'active', progress: 25 }

  // @ts-expect-error Matching variant fields do not create a Rust wrapper.
  const state: UploadState = plain
  // @ts-expect-error Even an enum without instance methods has a private brand.
  const instance: UploadState.Instance = {}
  expectTypeOf(Phase.clone_of({ type: 'waiting' })).toEqualTypeOf<Phase>()

  expectTypeOf(state).toEqualTypeOf<UploadState>()
  expectTypeOf(instance).toEqualTypeOf<UploadState.Instance>()
})

test('keeps structurally identical Rust enums distinct', () => {
  const upload = UploadState.new(25)
  const download = DownloadState.new(25)

  expectTypeOf<UploadState.Plain>().toEqualTypeOf<DownloadState.Plain>()
  // @ts-expect-error UploadState and DownloadState have distinct private brands.
  const wrongDownload: DownloadState = upload
  // @ts-expect-error Neither enum's brand is assignable to the other's.
  const wrongUpload: UploadState = download
  // @ts-expect-error Instance types also preserve each Rust enum's identity.
  const wrongInstance: DownloadState.Instance = upload

  expectTypeOf(wrongDownload).toEqualTypeOf<DownloadState>()
  expectTypeOf(wrongUpload).toEqualTypeOf<UploadState>()
  expectTypeOf(wrongInstance).toEqualTypeOf<DownloadState.Instance>()
})
