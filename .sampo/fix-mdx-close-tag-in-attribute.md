---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes a parsing error when a MDX attribute contained the closing tag of itself, e.g. `<Component attr="</Component>">`. The parser would incorrectly treat the `</Component>` as the closing tag of the component, instead of part of the attribute value.
