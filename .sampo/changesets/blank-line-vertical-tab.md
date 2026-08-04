---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a line holding only a vertical tab or form feed counting as a blank line, which split paragraphs and let a definition run past its destination.
