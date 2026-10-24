---
npm/satteri: patch
---

Added the `MdastVisitorFn` and `HastVisitorFn` type exports, so a visitor shared by several node types can be typed once, for example `MdastVisitorFn<Paragraph | Blockquote>`.
