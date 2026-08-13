---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed emphasis being parsed around a `~` when GFM is disabled, so `a*~*` now stays plain text.
