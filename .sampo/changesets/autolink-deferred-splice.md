---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed the text after a GFM autolink being mangled when the URL ends on a character reference or a backslash, which could decode the wrong character, report an overlapping position, or swallow the inline HTML or emphasis that followed.
