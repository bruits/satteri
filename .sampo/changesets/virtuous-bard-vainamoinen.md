---
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixed inline `style` custom properties (`--*`) being lowercased in MDX, which broke `var()` references to case-sensitive names like `--tmLabel`
