---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed links and reference definitions whose parenthesized title holds an unescaped `(`, as in `[a](* (())`, not being parsed as links.
