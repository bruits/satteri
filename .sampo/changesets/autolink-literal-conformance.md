---
cargo/satteri-pulldown-cmark: patch
---

Improved automatic linking of bare URLs and emails to match GitHub more closely, including uppercase schemes like `HTTP://` and `WWW.`, `www` hosts without a second dot, trailing punctuation, and not linking inside existing link text.
