---
cargo/satteri-pulldown-cmark: patch
---

Fixed the event parser with `ENABLE_ATTRIBUTES` leaving a heading's `{...}` as text when it follows an inline element.
