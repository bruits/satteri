---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-napi: minor
npm/satteri: minor
---

Add a `singleDollar` math option to disable single-dollar inline math (`$x$`) while keeping multi-dollar math (`$$...$$` and `$$` block fences). Mirrors remark-math's `singleDollarTextMath: false`, so prose that uses `$` for currency (e.g. `$50 to $100`) is no longer mis-parsed as math. Enable via `features: { math: { singleDollar: false } }`.
