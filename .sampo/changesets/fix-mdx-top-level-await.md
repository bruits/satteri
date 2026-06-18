---
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixes a parse error when an MDX expression uses top-level `await`, such as `<Card data={await getData()} />`.
