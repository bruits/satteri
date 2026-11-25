---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Added a `cjkFriendlyEmphasis` feature that lets `*` and `_` emphasis open and close next to CJK punctuation, following the [CommonMark CJK-friendly amendments](https://github.com/tats-u/markdown-cjk-friendly/blob/main/specification.md) (`これは**「重要」**です`).
