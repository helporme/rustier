/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { expect, it } from 'vitest'
import { Clone } from '$rust/core/clone'
import { From } from '$rust/core/convert'
import * as contracts from './contracts.rs'
import { Read as OtherRead } from './other_contracts.rs'

it('exports non-generic descriptors from standard and trait-only modules', () => {
  expect(Clone.$trait).toBe('core::clone::Clone')
  expect(Clone.$args).toEqual([])
  expect(contracts.Read.$args).toEqual([])
  expect(typeof Clone).toBe('object')
  expect(Object.isFrozen(Clone)).toBe(true)
  expect(Object.isFrozen(Clone.$args)).toBe(true)
  expect(contracts.Read.$trait).not.toBe(OtherRead.$trait)
  expect(Object.keys(contracts).sort()).toEqual(['Convert', 'Probe', 'Read'])
})

it('constructs discovered primitive and borrowed-string specializations', () => {
  for (const key of ['u32', 'f64', 'String', '&str'] as const) {
    expect(From(key)).toEqual({
      $trait: 'core::convert::From',
      $args: [key],
    })
  }
  expect(contracts.Probe('str').$args).toEqual(['str'])
  expect(contracts.Convert('u32').$args).toEqual(['u32'])
  expect(contracts.Convert('String').$args).toEqual(['String'])
})

it('rejects unavailable specializations, arity errors, and non-type keys', () => {
  for (const args of [[], ['u64'], ['u32', 'String'], [null], [42]]) {
    expect(() => Reflect.apply(From, undefined, args)).toThrow(TypeError)
  }
})

it('maps generated descriptor declarations back to trait definitions', () => {
  const map = JSON.parse(
    readFileSync(
      '.rustier/types/src/trait-descriptors/contracts.rs.d.ts.map',
      'utf8',
    ),
  )
  expect(map.names).toEqual(
    expect.arrayContaining(['Read', 'Convert', 'Probe']),
  )
  expect(
    map.sources.some((source: string) => source.endsWith('/contracts.rs')),
  ).toBe(true)
})
