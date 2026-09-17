import { expect, it } from 'vitest'
import { Opaque, Vault, opaque_value } from '../automatic-coercion/test.rs'

it('rejects input objects for an opaque Clone argument and field setter', () => {
  expect(() => opaque_value({ value: 42 } as never)).toThrow()
  expect(() => Vault.of({ value: { value: 42 } } as never)).toThrow()
  const vault = Vault.of({ value: new Opaque(7) })
  expect(() => { vault.value = { value: 42 } as never }).toThrow()
  expect(opaque_value(vault.value)).toBe(7)
})
