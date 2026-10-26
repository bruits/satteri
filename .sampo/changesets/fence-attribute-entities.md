---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed braces written as character references, such as `&#123;.a&#125;`, being read as a fenced code block's attributes.
