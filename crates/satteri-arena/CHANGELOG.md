# satteri-arena

## 0.3.2 — 2026-09-05

### Patch changes

- [af7e7ed](https://github.com/bruits/satteri/commit/af7e7ed74db87f163c22094a9a5790e1abdaf4e7) Made parsing, HTML rendering, and tree building faster, by 3% to 35% depending on the document and output, with the largest gains on small documents. — Thanks @Princesseuh!
- [af7e7ed](https://github.com/bruits/satteri/commit/af7e7ed74db87f163c22094a9a5790e1abdaf4e7) Made `position: false` faster and fixes cases were it could lower performance by accident compared to enabling positions. — Thanks @Princesseuh!
- [af7e7ed](https://github.com/bruits/satteri/commit/af7e7ed74db87f163c22094a9a5790e1abdaf4e7) Made `markdownToMdast`, `markdownToHast`, and the MDX tree functions faster and less memory-hungry, with the largest gains on text-heavy and non-ASCII documents. — Thanks @Princesseuh!

## 0.3.1 — 2026-08-19

### Patch changes

- [1a052cf](https://github.com/bruits/satteri/commit/1a052cfc0a295d7adbbcd3c9a1173d8a1b34598e) Improved performance. — Thanks @Princesseuh!

## 0.3.0 — 2026-08-18

### Minor changes

- [eeb7f07](https://github.com/bruits/satteri/commit/eeb7f0778a7af229fd592dd027ddfe0723ba2b26) Faster parsing, MDX compilation, and plugin execution. — Thanks @Princesseuh!
- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Faster across the board: parsing is ~10% cheaper, editing the tree from plugins now costs proportionally to how much you change rather than how big the document is (3 edits on a 115KB document: ~160µs → under 50µs), reading nodes inside plugins is 40-75% faster, and memory stays flat under sustained workloads. — Thanks @Princesseuh!

### Patch changes

- [c9985d9](https://github.com/bruits/satteri/commit/c9985d93b5ee23aff07491360be83d4a3412f18b) Fixed `development: true` line and column numbers, missing-component references, and MDX parse error locations being wrong in documents with multibyte or emoji characters. — Thanks @Princesseuh!
- [0d26ea6](https://github.com/bruits/satteri/commit/0d26ea6d68a29d4de8419423e030076244348c22) Changed the minimum supported Rust version to 1.85, as these crates now build on the 2024 edition. — Thanks @Princesseuh!
- [2e3ed23](https://github.com/bruits/satteri/commit/2e3ed23aa0e2489c4ce667cb39eb29259664692d) Faster Markdown-to-HTML rendering, most noticeably on prose-heavy documents where GFM autolink scanning dominated: a 200KB CommonMark document renders about 7% faster end to end. — Thanks @Princesseuh!
- [6696c1c](https://github.com/bruits/satteri/commit/6696c1c28b3024c5c8df760cc5af51dd713663fc) Fixed `position` offsets being wrong in documents with multibyte characters. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed documents that use standalone carriage returns (`\r`) as line endings parsing differently from documents that use `\n`. Values such as inline code and definition titles now keep the document's own line endings instead of always reporting `\n`. — Thanks @Princesseuh!

## 0.2.2 — 2026-06-29

### Patch changes

- [c6a9088](https://github.com/bruits/satteri/commit/c6a908875ae5161c86c592388a55f9caca9ed35b) Fixes plugin `ctx.source` being polluted with duplicated, concatenated content appended after the original document. — Thanks @Princesseuh!

## 0.2.1 — 2026-06-02

### Patch changes

- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Fixes Markdown plugins that return raw Markdown or HTML (`{ raw }` / `{ rawHtml }`) sometimes inserting unnecessary nested `root` nodes into the MDAST tree. — Thanks @Princesseuh!

## 0.2.0 — 2026-05-18

### Minor changes

- [43b5d8e](https://github.com/bruits/satteri/commit/43b5d8ed221591de11cf19008be09413425c9612) Republish with new public API: `LineIndexCursor` second lifetime parameter, `Arena::cp_offsets`, `LineIndexCursor::byte_to_cp_offset`, `ArenaBuilder::sort_current_pending_children_by_source_order`. — Thanks @Princesseuh!

## 0.1.4 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!

## 0.1.3 — 2026-04-27

### Patch changes

- [0f7ad25](https://github.com/bruits/satteri/commit/0f7ad259366f3bdc82a19a319625d3ffebd8edda) Expose `Arena::replace_node_with_children` and the `ArenaBuilder` helpers `last_sibling_id`, `sort_current_pending_children_by_start_offset`, and `update_leaf_full`. — Thanks @Princesseuh!

