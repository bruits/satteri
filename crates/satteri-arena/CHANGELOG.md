# satteri-arena

## 0.2.0 — 2026-05-18

### Minor changes

- [43b5d8e](https://github.com/bruits/satteri/commit/43b5d8ed221591de11cf19008be09413425c9612) Republish with new public API: `LineIndexCursor` second lifetime parameter, `Arena::cp_offsets`, `LineIndexCursor::byte_to_cp_offset`, `ArenaBuilder::sort_current_pending_children_by_source_order`. — Thanks @Princesseuh!

## 0.1.4 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!

## 0.1.3 — 2026-04-27

### Patch changes

- [0f7ad25](https://github.com/bruits/satteri/commit/0f7ad259366f3bdc82a19a319625d3ffebd8edda) Expose `Arena::replace_node_with_children` and the `ArenaBuilder` helpers `last_sibling_id`, `sort_current_pending_children_by_start_offset`, and `update_leaf_full`. — Thanks @Princesseuh!

