---
cargo/satteri-pulldown-cmark: minor
npm/satteri: minor
---

Added the `attributes` feature (`Options::ENABLE_ATTRIBUTES`), which reads curly-brace attributes on fenced code blocks (` ```js {.class #id} `) and applies them to the `<code>`, as markdown-it-attrs does. It also enables heading attributes.
