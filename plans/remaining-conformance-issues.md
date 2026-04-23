# Plan: remaining MDAST/HAST conformance issues

Session snapshot: **98.8% MDAST / 99.3% HAST** against the Astro docs (2417 `.mdx` files). The remaining gap is ~30 files with a long tail of unrelated issues. This plan breaks them into tractable work items so the tail can be closed when there's appetite for it.

Each item is scoped to stand alone — tackle any in isolation.

Diagnostic tooling left in `packages/satteri/test/conformance/`:
- `docs-check.mjs` — full run across `../docs/**/*.mdx`, prints MDAST/HAST pass rates
- `classify.mjs` — first-shape bucketing of MDAST failures
- `classify_hast.mjs` — HAST-only failures (those that survive when MDAST is already equal)
- `drill_root.mjs` — deeper per-bucket walk that picks an example file per bucket

---

## Item A — `listItem.spread` is always `false` (≈10 files)

### Symptom

```json
// REF
{ "type": "listItem", "spread": true, "children": [...code block..., ...details...] }
// SAT
{ "type": "listItem", "spread": false, ...same children... }
```

Pattern that triggers it: a list item contains multiple block-level children and any of those children has a blank line **inside** it (typically a multi-line flow JSX block `<details>...\n\n...\n</details>` or a paragraph followed by a fenced code block).

### Why

Remark/`mdast-util-from-markdown` sets `listItem._spread = true` during event post-processing when `firstBlankLineIndex` is set before the current line (see `mdast-util-from-markdown/lib/index.js` around line 330 — the `if (firstBlankLineIndex && (!lineIndex || firstBlankLineIndex < lineIndex))` branch). The flag is later emitted as `listItem.spread`.

Satteri's `arena_build.rs` emits `ListItemData { checked: 2, spread: false }` unconditionally (line ~677 and ~1320). No code path ever sets `spread: true`.

### Why it was left

The natural spots to update this are (a) during `ListItem` closing in the firstpass, or (b) during arena rebuild when we materialise the listItem node. Both need a state bit threaded through — "has this item seen a blank line between block-level children?"

I made a partial attempt in this session: I propagate `self.last_line_blank = true` through `parse_block` whenever a multi-line flow JSX/expression is consumed (firstpass.rs `contains_blank_line` helper). But `last_line_blank` only flips the enclosing **list's** `is_tight` — there's no per-`listItem` flag.

### Fix sketch

1. Track a `has_blank_between_blocks` bool on the current listItem tree node (or pass it as a parameter alongside `self.last_line_blank`).
2. Set it to `true` in two places during firstpass:
   - When `parse_block` observes a blank line between two block-level children of the item.
   - When a multi-line flow JSX/expression is consumed and `contains_blank_line` reports blank lines (the hook I already added).
3. In `arena_build.rs` (where `ListItemData { ..., spread: false }` is written), read the flag and emit `spread: true` when set.

### Open questions

- How is the flag propagated from firstpass tree into arena_build? Simplest: extend `ItemBody::ListItem(indent)` to `ItemBody::ListItem(indent, spread)` and encode it there. More intrusive but keeps state where it's generated.
- What exactly counts as "blank line between block-level children"? Test matrix:
  - `- para\n\n  code` → spread=true ✓
  - `- para\n  text` → spread=false
  - `- para\n\n  para2` → spread=true ✓
  - `- <details>\n\n  body\n\n  </details>` → spread=true (this is the session's repro)
  - `- <details>body</details>` → spread=false
- Does GFM task list's checkbox emission path need to preserve the flag too? (See `ItemBody::TaskListMarker` handler in arena_build.rs line 1318 — it rewrites `ListItemData` and would need to merge.)

### Estimate

~60 LOC in firstpass + ~20 LOC in arena_build + tests.

### Reference

- `node_modules/.pnpm/mdast-util-from-markdown@*/lib/index.js` — `prepareList` function (search `_spread`).

---

## Item B — `list` child-count diverges on Steps + indented code (≈9 files)

### Symptom

`src/content/docs/en/tutorial/6-islands/4.mdx` and similar files use:

```mdx
<Steps>
1.  First item.

2.  Second item.

    ```ts
    // indented 4 spaces under the `2.` marker
    ```

3.  Third item.
</Steps>
```

Remark parses it as `list(3 items)`. Satteri parses it as `list(1 item)` where the "single" item swallows items 2 and 3 as a nested sublist.

### Why

CommonMark's list-item continuation rule: content on subsequent lines must be indented to at least the column of the first non-marker character. `1.  First` has its first content column at 4 (two spaces after the `.`), so continuation lines must have ≥4 spaces of leading whitespace.

The indented ` ```ts ` fence at column 4 qualifies as item-2 continuation. The blank line between is fine. But item-3's `3.` marker at column 0 should start a new item — satteri seems to keep consuming.

My best guess: the interaction is that the fence opens while we're at an unusual container spine state, and the fence-body scanning doesn't re-check for list-item markers on close-fence exit. Needs tracing.

### Fix sketch

1. Write a minimal failing repro by narrowing `6-islands/4.mdx` line by line — my existing `drill_list_file.mjs` (removed at session end) had the right methodology. Start with just the `<Steps>` + `1.` + `2.` with the indented code + `3.` triple; strip everything else.
2. Trace `firstpass.rs::parse_block` at the transition from the fenced code block back into list-item continuation scanning. Compare to micromark's `commonmark-list.js` at the same transition.
3. Likely fix is in either `parse_fenced_code_block`'s exit path or the following `parse_block` iteration's `scan_containers` — check whether the ≥4-space indent of item-2 is correctly subtracted before checking for a sibling `3.` marker.

### Open questions

- Is this a generic CommonMark list bug, or specific to the `<Steps>` MDX wrapper interaction? A non-MDX repro would clarify.
- Does pulldown-cmark upstream have the same behavior? Worth diffing our `firstpass.rs` against upstream for this code path.

### Estimate

Unknown — could be a 10-line off-by-one or could unearth a larger list-parsing assumption. Budget half a day.

### Reference

- `node_modules/.pnpm/micromark-core-commonmark@*/lib/list.js` — list-item tokenizer.
- `node_modules/.pnpm/mdast-util-from-markdown@*/lib/index.js` — `prepareList`.

---

## Item C — `mdxFlowExpression` leading-indent stripping (2 files)

### Symptom

```mdx
{/* hello
 - line2
*/}
```

Remark value: `"/* hello\n- line2\n*/"` (stripped the leading space on continuation).
Satteri value: `"/* hello\n - line2\n*/"` (kept it).

The rule is NOT trivial "strip all leading whitespace". Testing shows remark strips **the minimum leading indent across all continuation lines** (similar to how Python docstring dedent works). For `{/* x\n    a\n     b\n*/}`, remark yields `"/* x\n  a\n   b\n*/"` — stripped 2, not 4.

### Why

`micromark-factory-mdx-expression` maintains an `initial` column tracking where the expression's `{` sits, and consumption is done line-by-line with that column subtracted from each continuation line. The opening `{` at column 0 still gets 1 space stripped because the first char after `{` (here `/`) establishes an initial content column of 1.

Actually, digging further: it's closer to "strip up to the column of the first post-`{` char on the opening line, but only where the continuation line has at least that many leading spaces." The real algorithm lives in `node_modules/.pnpm/micromark-factory-mdx-expression@*/dev/index.js`.

### Fix sketch

In `firstpass.rs::parse_mdx_jsx_flow` (which also handles flow expressions), the current logic copies the raw span verbatim. We'd need to:

1. Record the column of the `{` opening on the first line.
2. Record the column of the first non-whitespace char after `{` on the opening line (call this `initial`).
3. For each continuation line, strip up to `initial` columns of leading whitespace.

Both flow expression (`{...}`) and JSX attribute expressions should follow the same rule.

### Open questions

- Does the rule apply when there's no content on the first line after `{` (i.e. `{\n foo\n}`)? Needs a reference test.
- Is the same rule used in `mdxFlowExpression` vs `mdxTextExpression`? Testing shows text expressions behave differently (single-line only, usually).

### Estimate

~50 LOC in `mdx.rs`, plus 5–10 test cases. Low-medium complexity but careful spec adherence required.

### Reference

- `node_modules/.pnpm/micromark-factory-mdx-expression@*/dev/index.js`.

---

## Item D — `\r\n` vs `\n` in inline text (2 files)

### Symptom

```json
// REF inline text node
{ "type": "text", "value": "\r\n" }
// SAT
{ "type": "text", "value": "\n" }
```

The file's source has CRLF line endings. Inline text where satteri should preserve `\r\n` between softbreaks is collapsed to `\n` somewhere.

### Why

Earlier in this session I removed CRLF→LF normalization from `append_code_text` and `append_html_line` (preserving CRLF in fenced code / html blocks / yaml — that fixed 1 visible file and several invisible ones). But inline text runs through a different path. Most likely: somewhere in `parse.rs` or the softbreak-producing path, a `\r` is stripped when a softbreak boundary is detected.

### Fix sketch

1. Find the specific `\r\n` case — likely a text node immediately before or after a hard break, or inside a paragraph where the preceding line ends in `\r\n`.
2. Grep `parse.rs` for `b'\r'` — there are a few places, mostly in tokenization. Identify which one is eating the `\r`.
3. Either keep the `\r` in the Text item's byte range, or emit it as a separate trailing `SynthesizeText("\r")`.

### Open questions

- Does the code currently treat `\r\n` as a single softbreak terminator and drop both, then the re-emission uses only `\n`? If so the fix may be in softbreak emission.

### Estimate

~20 LOC once located. Low complexity, but needs careful tracing.

---

## Item E — structural 1-offs at root level (4 files)

Each of these is its own small bug. Worth a short individual investigation; low priority until listItem.spread and list 3v1 above are settled since those cascade.

1. **`ar/basics/layouts.mdx`** — `root child-count 46 vs 44  mdxFlowExpression→heading`. Satteri has 2 extra children where remark has one mdxFlowExpression. Likely the expression is being split into text+expression+text or similar.
2. **`ko/guides/cms/builderio.mdx`** — `root child-count 73 vs 27`. Large split difference. Likely a boundary issue in one of the MDX constructs that cascades — needs narrowing.
3. **`pl/concepts/islands.mdx`** — `root 50 vs 21 heading→undefined`. Similar cascade pattern to the Korean file above. Probably same root cause.
4. **`pl/basics/layouts.mdx`** — `root 42 vs 41  mdxFlowExpression→containerDirective(note)`. Satteri emits a directive where remark emits a flow expression. Probably an `{/* note */}` comment misidentified.

### Approach

For each: extract minimal failing span, drop into a test, bisect against surrounding content until only the construct that matters remains. Fix in place.

### Estimate

30 min – 1 hour per file.

---

## Item F — miscellaneous

- **`de/guides/cms/index.mdx`** (1 file): paragraph child-count 4 vs 3 — remark keeps `text "),"` + `text " um…"` as separate Text nodes; satteri merges them into one. This is the opposite case from the entity-merge fix. Investigate whether remark's autolink transform emits two text chunks that `emit_text_merging` shouldn't merge (because they arise from a URL-trimming-`)` side-path).
- **`ja/guides/ecommerce.mdx`** (1 file): Japanese directive name — the name truncation was fixed upstream but the file still fails on a different, downstream divergence. Worth re-drilling now that the upstream name issue is resolved.

---

## Suggested order

1. **Item A (listItem.spread)** — clear spec, deterministic fix, 10-file impact.
2. **Item C (mdxFlowExpression indent)** — clear spec from micromark-factory-mdx-expression, 2-file impact but affects common MDX constructs beyond the test set.
3. **Item B (list 3v1)** — largest unknown; budget accordingly.
4. **Items D, E, F** — pick off opportunistically.

After A–C land, MDAST should sit at **≥99.5%** against the docs corpus. D+E+F can push it the rest of the way.

## Related follow-up (not blocking)

The `listItem.spread` work has a spiritual cousin on the HAST side: the list rendering helper (`convert_children_unwrap_paragraphs_task`) already unwraps paragraphs based on tightness. Once per-item spread is set correctly, some HAST rendering that currently falls back to "always unwrap" may need to branch on the item's `spread` flag. Worth a quick re-run of `docs-check.mjs` after Item A to confirm HAST numbers don't regress.
