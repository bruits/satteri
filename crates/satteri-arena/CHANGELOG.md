# satteri-arena

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

