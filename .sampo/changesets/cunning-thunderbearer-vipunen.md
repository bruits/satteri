---
cargo/satteri-ast: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed `rawHtml: true` losing code-fence `data.lang` and `data.meta` in `markdownToHast` and MDX compilation, so HAST plugins can read fence options.
