---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a link title being accepted with no whitespace after a `<...>` destination, so `[a](<u>"t")` is now plain text like in remark.
