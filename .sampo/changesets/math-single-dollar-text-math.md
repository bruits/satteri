---
cargo/satteri-pulldown-cmark: minor
cargo/satteri-napi: minor
npm/satteri: minor
---

Adds granular `features.math` control. `singleDollarTextMath: false` keeps single-`$` constructs as literal text (so prose can carry currency like "$50 to $100") while `$$ ... $$` still parses as display math.

```ts
markdownToHtml(source, {
  features: { math: { singleDollarTextMath: false } },
});
```
