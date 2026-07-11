---
cargo/satteri-napi: minor
npm/satteri: patch
---

Trims per-compile overhead on the plugin path; pipelines with only HAST plugins now parse, extract frontmatter, and convert in a single native call.
