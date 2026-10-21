---
cargo/satteri-pulldown-cmark: patch
---

Fixed directive attributes dropping an explicit `class=` written next to `.class` shortcuts and keeping the first of two ids, and changed directive and heading attributes to keep the order they are written in, as `remark-directive` does.
