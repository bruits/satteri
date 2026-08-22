---
npm/satteri: patch
cargo/satteri-arena: patch
cargo/satteri-pulldown-cmark: patch
---

Made `position: false` actually faster to parse. It previously cost about 3% more than keeping positions, and now saves about 19%.
