import { expect, it } from 'vitest'
import {
  make_draft_content,
  make_draft_document,
  make_published_document,
  set_published_content_value,
} from './test.rs'

it('replaces Document::Draft.content because DraftContent is cloneable', () => {
  const document = make_draft_document(10)
  const replacement = make_draft_content(20)
  if (document.type !== 'draft') throw new Error('expected Document::Draft')

  document.content = replacement
  replacement.value = 30

  expect(document.content.value).toBe(20)
})

it('returns Document::Published.content as a live readonly projection', () => {
  const document = make_published_document(10)
  if (document.type !== 'published')
    throw new Error('expected Document::Published')

  set_published_content_value(document.content, 20)

  expect(document.content.value).toBe(20)
})
