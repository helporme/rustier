import { expect, test } from 'vitest'
import { rs } from 'rustier-next'

test('handles compose with Result returns and release borrows on errors', () => {
  const message = rs`Ok(handy_crate::Message("Hello"))`
  const fail = () => rs`
    let message: &mut handy_crate::Message = ${message};
    message.0 = "Changed";
    Err::<(), _>("failed")
  `

  expect(fail).toThrow('failed')
  expect(rs`let value: &handy_crate::Message = ${message}; Ok(value.0.to_owned())`).toBe('Changed')
  const transferred = rs`let value: handy_crate::Message = ${message}; Ok(value)`
  expect(() => rs`let value: &handy_crate::Message = ${message}; Ok(value.0.to_owned())`).toThrow(/moved/i)
  expect(rs`let value: &handy_crate::Message = ${transferred}; value.0.to_owned()`).toBe('Changed')
})
