---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-napi: patch
npm/satteri: patch
---

Fixed a `:directive` after an invalid bare URL being destroyed instead of parsed, as in `http://my_app.localhost:3000/admin`.
