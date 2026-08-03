---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a bare URL or email not linking when a character reference supplies its first character, as in `&#104;ttp://example.com`.
