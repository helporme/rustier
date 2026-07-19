import { expect, it } from 'vitest'
import { no_args_no_return } from './test.rs'

it('calls a function without arguments or return value', () => {
  expect(no_args_no_return()).toBeUndefined()
})
