---
npm/satteri: patch
cargo/satteri-arena: patch
cargo/satteri-ast: patch
---

Made `markdownToMdast`, `markdownToHast`, and the MDX tree functions faster and less memory-hungry, with the largest gains on text-heavy and non-ASCII documents.
