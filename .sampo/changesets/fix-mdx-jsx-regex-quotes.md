---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes quotes inside a regex in an MDX JSX attribute (e.g. `ins={[/icon="[^"]+"/g]}`) causing a parse error.
