---
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixed MDX files that declare a layout via `export { default } from ...` or `export default` not rendering at runtime.
