---
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixed plugin-inserted elements being emitted as literal JSX tags, instead of going through `_components`, when another plugin-inserted node was marked as explicit JSX.
