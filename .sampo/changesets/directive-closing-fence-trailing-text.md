---
cargo/satteri-pulldown-cmark: patch
npm/satteri: patch
---

Fixed container directives closing on a line such as `::: card`. Like remark-directive, a closing fence can now only be followed by whitespace, so the rest of that line stays as content.
