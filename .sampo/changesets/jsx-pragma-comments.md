---
cargo/satteri-mdxjs: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `jsx: true` output not saying which JSX runtime to use, so a bundler compiling the JSX ignored `jsxImportSource` and the pragma options.
