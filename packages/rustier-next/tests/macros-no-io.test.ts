import { test } from 'vitest'
import { rs } from 'rustier-next'

test('macros without input nor output just runs', () => {
  rs`2+2;`
  rs`4+2;`
  rs`2+5;`
})
