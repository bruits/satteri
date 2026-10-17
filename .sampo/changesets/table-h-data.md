---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed `data.hName`, `data.hProperties` and `data.hChildren` on table rows and cells being ignored: they now apply to the `<tr>`, `<th>` and `<td>`, as `mdast-util-to-hast` does.
