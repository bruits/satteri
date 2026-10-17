---
cargo/satteri-pulldown-cmark: minor
npm/satteri: minor
---

Added inline elements to the `attributes` feature: a `{...}` written right after emphasis, strong, a code span, a link, an image or strikethrough (`*em*{.class}`) applies to that element, as markdown-it-attrs does, and blocks that follow one another merge.
