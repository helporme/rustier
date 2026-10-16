import { expectTypeOf, test } from 'vitest'
import {
  Meter,
  Receipt,
  Status,
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

test('declares only the successful payload as the return type', () => {
  expectTypeOf(read_number()).toEqualTypeOf<number>()
  expectTypeOf(read_text()).toEqualTypeOf<string>()
  expectTypeOf(read_flag()).toEqualTypeOf<boolean>()
  expectTypeOf(save()).toEqualTypeOf<void>()
  expectTypeOf(issue_receipt(1)).toEqualTypeOf<Receipt>()
  expectTypeOf(read_status(false)).toEqualTypeOf<Status>()
  expectTypeOf(aliased_reading()).toEqualTypeOf<number>()
})

test('substitutes generic Result payloads in method declarations', () => {
  const meter = make_meter(1)

  expectTypeOf(Meter('i32').accepted(3)).toEqualTypeOf<number>()
  expectTypeOf(meter.read()).toEqualTypeOf<number>()
  expectTypeOf(meter.replace(2)).toEqualTypeOf<number>()
})

test('retains the enum type for nested Result values and fields', () => {
  expectTypeOf(nested_outcome()).toEqualTypeOf<ReturnType<typeof make_report>['outcome']>()
  expectTypeOf(nested_outcome()).toExtend<Parameters<typeof describe_outcome>[0]>()
})
