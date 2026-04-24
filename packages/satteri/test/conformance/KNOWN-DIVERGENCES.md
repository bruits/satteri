# Known divergences from remark-gfm

This document lists the handful of cases where satteri knowingly produces
different output than remark + remark-gfm + remark-rehype. It is scoped to
divergences we've investigated and decided to live with (for now) — not
bugs we'd want to ship. Each entry links back to a reproducing test and
explains what would need to change to close the gap.

Conformance snapshot at the time of writing:

| Corpus | MDAST | HAST |
| --- | --- | --- |
| `astro/docs` (2417 MDX files) | 2417 / 2417 (100.0%) | 2417 / 2417 (100.0%) |
| `cloudflare-docs` (8512 MDX files) | 8503 / 8512 (99.9%) | 8504 / 8512 (99.9%) |

These numbers come from `packages/satteri/test/conformance/docs-check.mjs`
and `cloudflare-docs-check.mjs`; re-run either to regenerate the
`DOCS-CHECK.md` / `CLOUDFLARE-DOCS-CHECK.md` reports in this directory.
Both runs pass `math: false` in `FEATURES` so the comparison mirrors the
reference pipeline (remark + remark-gfm + remark-mdx, no math plugin).

## Ignored Rust tests

Five live in `crates/satteri-pulldown-cmark/tests/suite/` and are marked
`#[ignore]` with a one-paragraph explanation inline. Running `cargo test
--workspace` reports them as `ignored`, not failed.

### `footnotes_test_16` — bracket + footnote interaction

**Input shape:** `My [cmark-gfm][^c].` / `My [cmark-gfm][c][^c].` repeated
with a trailing `[otherlink[^c]]: https://…` reference block.

**Divergence:** remark resolves `[cmark-gfm][^c]` as the shortcut link
`[cmark-gfm]` (literal) followed by footnote `[^c]`; we resolve
`[cmark-gfm][c][^c]` by consuming `[cmark-gfm][c]` as a reference link
(URL from `[c]: …`) and then the footnote. Shows up when the same
identifier has both a link definition and a footnote definition.

**Fix direction:** the `MaybeLinkClose` handler in
`crates/satteri-pulldown-cmark/src/parse.rs` already has a
footnote-first short-circuit for `[^X][…]`. Extend it to recognise the
inverse shape `[X][Y][^Z]` — when the repeated label `Y` resolves to a
link definition AND a trailing `[^Z]` is a defined footnote, treat the
first bracket pair as a shortcut and emit link + footnote separately.

### `footnotes_test_20` — outdated expected HTML

**Input shape:** indented GFM tables at the document root mixed with
footnote references inside a blockquote.

**Divergence:** after the block-in-container work we now recognise the
nested tables correctly (same as remark-gfm), but the hardcoded
`expected` HTML in the test dates from the older permutation and no
longer matches remark either.

**Fix direction:** regenerate `expected` from the current remark output
and unignore. The blockquote+footnote-reference portion is a separate,
orthogonal gap.

### `regression_test_175` — blank-line separator after indented code

**Input shape:** `*` list item followed by an indented code block then an
HTML block.

**Divergence:** remark emits a blank line (a trailing `\n\n`) between
the `<pre>` and the HTML block; we emit a single `\n`. Both are valid
HTML, but byte-compare fails.

**Fix direction:** cosmetic — the HTML renderer in `satteri-ast::hast`
collapses consecutive block boundaries. Changing the
code-block→html-block boundary to keep a blank line would match remark;
needs care not to regress other block adjacency cases.

### `regression_test_197` — inline-link paren balance

**Input shape:** `[40](https://rust.org/something%3A((((…))))…)` with 40
nested parens in the URL body.

**Divergence:** we already cap paren depth at 32 (same as remark), so
both parsers reject the inline link. The difference is what happens next
— remark leaves the whole `[40](…)` as literal text; our GFM
autolink-literal post-pass re-tokenises the URL inside the parens and
emits a nested `<a>`.

**Fix direction:** when `scan_inline_link` fails on paren balance, mark
the surrounding `(…)` span as autolink-suppressed so the URL stays as
text. The span boundaries are already known at rejection time.

### `regression_test_198` — soft-break inside task-list item

**Input shape:** `- [x]<TAB><TAB>\n\\\n-\n` — task-list marker followed
by trailing tab whitespace, then a `\` line, then a setext-style `-`.

**Divergence:** remark emits a trailing `\n` before the `\` text inside
the `<li>`; we skip it because our task-list cell's trailing whitespace
consumer eats the newline.

**Fix direction:** the task-list cell normaliser in `firstpass.rs`
should preserve trailing `\n` when a hard-break character (`\`) follows
on the next line. Narrow fix; shares state with the setext/hr
interaction so probably worth a quick regression sweep after changing.

## Remaining Cloudflare docs drift (9 MDAST / 8 HAST files)

All remaining divergences come from two narrow parser-internals gaps.

### 8 files — HTML-like `<table><tr><td>` layout with hard-break markers

- 7× `src/content/changelog/waf/*-waf-release.mdx`
- 1× `src/content/docs/analytics/graphql-api/getting-started/authentication/index.mdx`

Each file has a literal HTML table (`<table><thead>…<tbody><tr><td>…</td></tr>`)
with mixed tab/space indentation across lines. At least one line in the
run of consecutive `<td>` tags ends with ≥2 trailing spaces (a markdown
hard-break marker). Remark-mdx treats hard-break-terminated lines as
paragraph content, and because subsequent JSX-only lines are valid
paragraph continuations, remark pulls the whole run into a single
inline-JSX paragraph with soft-break text nodes and a `break` node.

Our block-level JSX scanner commits to flow classification as soon as
it sees a JSX-only line, one line at a time. Matching remark's
collapse-on-hard-break behavior needs a **lookahead pre-pass**: before
emitting a JSX flow block, scan all consecutive JSX-only lines until a
blank line; if any carries a hard-break marker, treat the whole run as
paragraph-inline instead.

A partial fix that only rejects the single hard-break line (tried and
reverted) leaves the other lines as flow — so the run ends up as
`flow + paragraph + flow` instead of the single paragraph REF produces.

Structural only — rendered HTML is equivalent in practice.

### 1 file — deep JSX attribute indent (add-split-tunnels-route)

- `partials/cloudflare-one/warp/add-split-tunnels-route.mdx`

Deeply nested `<SubtractIPCalculator defaults={{ base: "…", subtract: [""] }}/>`
sits inside two `<Tabs><TabItem>` wrappers inside a list item inside a
`:::note`. The attribute-expression's tab-indented continuation lines
preserve 1 extra tab level in our output vs. remark.

The strip algorithm works column-correctly for single-wrapper nesting,
but deep multi-level JSX wrappers introduce additional implicit indent
that `container_content_col` doesn't track (JSX is not on the container
spine in our model). An attempt to derive an "element excess" strip
from `element_column - container_content_col` fixed this file but
regressed 8 simpler cases, so was reverted.

**Fix direction:** proper fix probably requires tracking a per-level
wrapper-indent contribution and threading it through the
strip-expression machinery, or adopting remark-mdx's full position
propagation. Not worth it for a single file.

## How to regenerate this snapshot

```sh
# 1. Rebuild the native binding so our mdast/hast reflect the current
#    Rust source.
pnpm -C packages/satteri build:native

# 2. Astro docs (2417 files, ~10 s).
pnpm -C packages/satteri exec node test/conformance/docs-check.mjs

# 3. Cloudflare docs (8512 files, ~25 s).
pnpm -C packages/satteri exec node test/conformance/cloudflare-docs-check.mjs
```

Both scripts write their report next to this file. If the totals change
materially, update the snapshot table at the top.
