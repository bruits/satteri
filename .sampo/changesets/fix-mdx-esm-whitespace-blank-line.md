---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fix MDX `import`/`export` blocks being broken by a following whitespace-only line. A line containing only spaces or tabs now ends the ESM block exactly like an empty line, instead of being consumed as a statement continuation (which produced a `Could not parse esm with oxc` error).
