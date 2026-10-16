import { expect, it } from 'vitest'
import { RuntimeError } from '$rust'
import { Counter, checked_add } from '../clone-and-borrow/test.rs'
import { checked_number } from '../std-generics/test.rs'
import {
  AccessFailure,
  Rejection,
  make_attempts,
  reject_access,
  reject_flag,
  reject_number,
  reject_pair,
  reject_request,
  reject_text,
  reject_unit,
} from './test.rs'

it.each([
  ['number', reject_number, 404],
  ['string', reject_text, 'offline'],
  ['false', reject_flag, false],
  ['unit', reject_unit, undefined],
  ['standard-library fixture string', () => checked_number(false), 'invalid'],
] as const)('throws the %s Err payload without wrapping it', (_name, reject, expected) => {
  expect.assertions(1)

  try {
    reject()
  } catch (error) {
    expect(error).toBe(expected)
  }
})

it('throws a usable non-Clone Rejection without requiring the Error trait', () => {
  expect.assertions(5)

  try {
    reject_request()
  } catch (error) {
    expect(error).toBeInstanceOf(Rejection)
    expect(error).not.toBeInstanceOf(Error)
    if (!(error instanceof Rejection)) throw error
    expect(error.code).toBe(503)
    expect(error.retryable()).toBe(true)

    error.code = 400

    expect(error.retryable()).toBe(false)
  }
})

it.each([false, true])('throws the selected AccessFailure variant (unavailable=%s)', unavailable => {
  expect.assertions(unavailable ? 2 : 3)

  try {
    reject_access(unavailable)
  } catch (error) {
    expect(error).toBeInstanceOf(AccessFailure)

    const failure = error as AccessFailure

    expect(failure.type).toBe(unavailable ? 'Unavailable' : 'Denied')

    if (failure.type === 'Denied') {
      expect(failure.resource).toBe('invoices')
    }
  }
})

it('executes each Result-returning method once and releases borrows after Err', () => {
  const attempts = make_attempts()

  expect.assertions(4)

  try {
    attempts.reject()
  } catch (error) {
    expect(error).toBe('rejected')
  }

  expect(attempts.count).toBe(1)
  expect(attempts.record()).toBe(2)
  expect(attempts.count).toBe(2)
})

it('clones an aliased owned argument and releases the mutable borrow after Err', () => {
  const counter = Counter.of({ value: 21 })

  expect.assertions(4)

  try {
    checked_add(counter, counter, false)
  } catch (error) {
    expect(error).toBe('rejected')
  }

  expect(counter.value).toBe(21)
  expect(checked_add(counter, counter, true)).toBe(42)
  expect(counter.value).toBe(42)
})

it('distinguishes runtime borrow errors from user Err and recovers after both', () => {
  const first = make_attempts()
  const second = make_attempts()

  expect.assertions(9)

  try {
    reject_pair(first, first)
  } catch (error) {
    expect(error).toBeInstanceOf(RuntimeError)
    expect(error).toBeInstanceOf(Error)
    if (!(error instanceof RuntimeError)) throw error
    expect(error.message.length).toBeGreaterThan(0)
  }

  expect(first.count).toBe(0)

  try {
    reject_pair(first, second)
  } catch (error) {
    expect(error).toBe('pair rejected')
  }

  expect(first.count).toBe(1)
  expect(second.count).toBe(1)
  expect(first.record()).toBe(2)
  expect(second.record()).toBe(2)
})
