---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed footnote links and IDs for identifiers containing non-ASCII characters or URL punctuation, which are now percent-encoded. `[^café]` links to `#user-content-fn-caf%C3%A9`, and an identifier holding a bare `%` no longer produces an invalid URL.
