---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixes inline math like `$\frac{-b}{2a}$` failing to compile in MDX. Braces inside `$...$` are now treated as math text, not a JSX expression.
