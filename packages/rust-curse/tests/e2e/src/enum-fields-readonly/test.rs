/// Cloneable content stored by a draft document.
#[derive(Clone)]
pub struct DraftContent {
    pub value: i32,
}

/// Non-cloneable content stored by a published document.
pub struct PublishedContent {
    pub value: i32,
}

/// Returns cloneable content for assignment to a draft document.
pub fn make_draft_content(value: i32) -> DraftContent {
    DraftContent { value }
}

/// Mutates published content through a live projection.
pub fn set_published_content_value(content: &mut PublishedContent, value: i32) {
    content.value = value;
}

/// A document whose content is writable in drafts and readonly after publication.
pub enum Document {
    Draft { content: DraftContent },
    Published { content: PublishedContent },
}

/// Returns a draft document with cloneable content.
pub fn make_draft_document(value: i32) -> Document {
    Document::Draft {
        content: DraftContent { value },
    }
}

/// Returns a published document with non-cloneable content.
pub fn make_published_document(value: i32) -> Document {
    Document::Published {
        content: PublishedContent { value },
    }
}
