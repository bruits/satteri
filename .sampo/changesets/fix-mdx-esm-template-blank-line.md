---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes a blank line inside a template literal or block comment in an MDX `import`/`export` causing an `Unterminated string` error. The blank line no longer ends the statement early.
