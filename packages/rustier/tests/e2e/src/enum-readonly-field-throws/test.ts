import { expect, it } from 'vitest'
import { make_draft_content, make_published_document } from './test.rs'

it('throws when replacing readonly Document::Published.content', () => {
  const document = make_published_document(10)

  expect(() =>
    Reflect.set(document, 'content', make_draft_content(30)),
  ).toThrow('Field is readonly on the active enum variant')
})
