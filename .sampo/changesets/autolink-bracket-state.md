---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed GFM autolinks getting the wrong URL, or being dropped entirely, when a `[` earlier in the paragraph belongs to a code span, inline HTML, a pointed autolink, or a link that never resolves.
