import { expect, it } from 'vitest'
import {
  Meter,
  Receipt,
  aliased_reading,
  describe_outcome,
  issue_receipt,
  make_meter,
  make_report,
  nested_outcome,
  read_flag,
  read_number,
  read_status,
  read_text,
  save,
} from './test.rs'

it('unwraps numeric, string, and false Ok payloads', () => {
  expect(read_number()).toBe(42)
  expect(read_text()).toBe('ready')
  expect(read_flag()).toBe(false)
})

it('returns undefined for Ok(())', () => {
  expect(save()).toBeUndefined()
})

it('returns a usable non-Clone Receipt instead of a Result wrapper', () => {
  const receipt = issue_receipt(23)

  expect(receipt).toBeInstanceOf(Receipt)
  expect(receipt.number).toBe(23)
  expect(receipt.label()).toBe('23')
  receipt.number = 24
  expect(receipt.label()).toBe('24')
})

it('returns both unit and record variants of a non-Clone Status', () => {
  expect(read_status(false).type).toBe('Ready')

  const pending = read_status(true)

  expect(pending.type).toBe('Pending')
  if (pending.type !== 'Pending') throw new Error('expected Status::Pending')
  expect(pending.position).toBe(3)
})

it('unwraps a type alias of standard Result', () => {
  expect(aliased_reading()).toBe(17)
})

it('substitutes Result payloads on generic associated and instance methods', () => {
  const meter = make_meter(12)

  expect(Meter('i32').accepted(5)).toBe(5)
  expect(meter.read()).toBe(12)
  expect(meter.replace(19)).toBe(12)
  expect(meter.read()).toBe(19)
})

it('keeps Result arguments as ordinary enum inputs', () => {
  expect(describe_outcome({ type: 'Ok', 0: 7 })).toBe('7')
  expect(describe_outcome({ type: 'Err', 0: 'offline' })).toBe('offline')
})

it('unwraps only the outer Result and returns an inner Err as an enum', () => {
  const outcome = nested_outcome()

  expect(outcome.type).toBe('Err')
  if (outcome.type !== 'Err') throw new Error('expected Result::Err')
  expect(outcome[0]).toBe('pending')
  expect(describe_outcome(outcome)).toBe('pending')
})

it('keeps Result-valued fields as ordinary enums', () => {
  const report = make_report()

  expect(report.outcome.type).toBe('Ok')
  expect(describe_outcome(report.outcome)).toBe('8')

  report.outcome = { type: 'Err', 0: 'missing' }

  expect(report.outcome.type).toBe('Err')
  expect(describe_outcome(report.outcome)).toBe('missing')
})
