# PR review notes: `fix/fuzzes-and-fixes`

This document orients a reviewer for a deep pass on the branch. It covers
intent, the load-bearing changes, hot spots for performance and code-quality
review, the testing strategy, and known limitations.

## TL;DR

The branch is a long-running "fuzz to convergence with remark" effort. It
started by randomly generating Markdown / GFM / MDX inputs, comparing
satteri's output to remark's, and grinding through the divergences. Along
the way the parser absorbed a lot of edge-case fixes, the HTML rendering
path was retired in favour of going through `satteri-ast`, the JSX flow vs.
inline classification was reworked, GFM autolink-literal got its own
post-pass, MDX JSX attribute indent stripping was rebuilt, and the test
expectations were updated to match remark wherever they previously
mirrored cmark-gfm.

End-state: 0 ignored Rust tests, 0 lint warnings, 100% conformance on the
Astro docs MDX corpus and 99.9% on Cloudflare docs (9 files documented as
known-non-blockers — see "known limitations" below).

## Branch stats

```
97 files changed, 15420 insertions(+), 8876 deletions(-)
```

Largest changes by file:

| File | LoC | What's in it |
| --- | --- | --- |
| `crates/satteri-pulldown-cmark/src/arena_build.rs` | 3227 (+2420) | Pulldown→arena driver and all post-passes (autolink-literal, directive label, mark-and-unravel, etc.) |
| `crates/satteri-pulldown-cmark/src/firstpass.rs` | 3922 (+1221) | Block-level parser (containers, list items, table-vs-list disambiguation, JSX flow detection) |
| `crates/satteri-pulldown-cmark/src/parse.rs` | 3278 (+515) | Inline parser (link/image close handling, footnote refs, math, MDX) |
| `crates/satteri-pulldown-cmark/src/mdx.rs` | 1584 (+581) | MDX JSX/expression scanning, attribute parsing, indent stripping |
| `crates/satteri-ast/src/convert.rs` | 1641 (+873) | mdast→hast converter (footnote ordering, ref resolution, position fix-ups) |
| `crates/satteri-mdxjs-rs/src/oxc_util_build_jsx.rs` | 2519 (+1595) | MDX JSX → JS (oxc-based) — substantial changes; orthogonal to the parser fixes, audit separately |

The deletions are mostly `crates/satteri-pulldown-cmark/src/html.rs` (a
678-line direct-to-HTML renderer we no longer need; HTML now goes through
`satteri-ast::mdast_to_html`) plus the auto-generated tests for spec
fixtures we dropped (`blockquotes_tags`, `definition_lists`,
`old_footnotes`).

## Architectural changes worth understanding before diving in

### 1. HTML rendering moved out of `satteri-pulldown-cmark`

`html.rs` and the `html` Cargo feature are gone. Anywhere we used to
`html::push_html(&mut out, Parser::new_ext(...))` now does
`let (arena, _) = parse(input, opts); satteri_ast::mdast_to_html(&arena)`.
The arena is the source of truth; HTML is one consumer of it.

**Why it matters for review**: every HTML test in this crate now
exercises the full parse → mdast arena → mdast→hast → render pipeline.
The parser and the AST converter are tightly coupled by this; a parser
regression often surfaces as an HTML diff.

### 2. `ENABLE_GFM` is now an umbrella flag

In `arena_build.rs::parse`, `ENABLE_GFM` is expanded at entry to
`ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS`. Callers that
pass `ENABLE_GFM` no longer need to remember to OR in the sub-flags.
Existing fine-grained flags still work the same.

### 3. GFM autolink-literal lives as a post-pass on the arena

`gfm_autolink_literal_pass` walks `Text` nodes after the main parse,
matches `http(s)://…` / `www.…` / `email@host` candidates, and splits the
text node into `Link` (or `mailto:` Link) + surrounding Text. This mirrors
`mdast-util-gfm-autolink-literal`'s post-transformer.

**Why a post-pass and not inline**: the GFM extension's matching rules
(prefix-character requirement, paren-balance trim-back, broken-bracket
suppression) are awkward to thread through pulldown-cmark's inline
tokenizer. A post-pass is straightforward and stays correct because
text-node boundaries are stable after the main parse.

### 4. Directive parsing has been substantially reworked

`packages/satteri/test/conformance/directive.test.ts` is the canonical
test surface. Container (`:::tip`), leaf (`::name[…]`), and text (`:name`)
directives are recognised; a few label-rewriting post-passes
(`directive_label_inline_code_pass`, `directive_label_jsx_pass`) exist to
match remark's behaviour of running inline parsing on the directive's
label string.

**Trade-off**: directives are interleaved with MDX so the post-passes
walk the arena multiple times. See "performance hot spots" below.

### 5. MDX JSX flow vs. inline classification

This was the largest fuzz/conformance gap. A line like `<td>foo</td>`
gets parsed first as a paragraph with inline JSX, then promoted to flow
by `mdx_mark_and_unravel` (port of `@mdx-js/mdx`'s plugin of the same
name). Multi-line JSX (`<X>\nfoo\n</X>`) is recognised as flow directly
in `firstpass.rs`. Indent stripping for multi-line JSX attribute
expressions lives in `mdx.rs::strip_expression_indent` /
`dedent_expression_continuation`.

The known cloudflare divergence in the "trailing whitespace as hard
break in a JSX-flow run" case is not covered by either path — see
"known limitations".

### 6. Footnote ordering moved into a single collection pass

`satteri-ast::convert::collect_refs` does a 2-pass sweep before any
hast emission: pass 1 collects link/image definitions and footnote
definition node ids, pass 2 walks the main flow plus referenced
definition bodies (BFS-ish) to assign source-order numbers matching
`remark-gfm`'s `state.footnoteOrder`. The numbers and per-id occurrence
counts are then threaded through hast emission via `ConvertCtx`.

**Why it matters**: the old converter computed footnote numbering
ad-hoc per emission site, which was both slower and wrong for nested
references. New code is one allocation up front and a HashMap lookup at
emit time.

## Hot spots for review

### Performance candidates

In rough order of how much they show up in profiles:

1. **`arena_build.rs::parse`'s post-passes**. After the main parse we
   run, in order: `merge_directive_port_splits` (only when directives
   on), `gfm_autolink_literal_pass`, `directive_label_inline_code_pass`,
   `directive_label_jsx_pass` (only when MDX + directives), the
   reference-link end-position fix-up, and `mdx_mark_and_unravel`. Each
   is a linear walk over the arena, but several allocate `Vec<u32>`
   buffers per iteration. Worth checking if any can be fused — they're
   currently independent for clarity.

2. **`split_text_with_autolinks` in `arena_build.rs`**. Replaces a
   single text node with a sequence of (Text, Link, Text, …) children.
   Allocation pattern: `Vec` per text node that contains a candidate.
   The `scan_autolink_literal` scanner does up to one byte of look-back
   per `h`/`w` start; it's tight but not micro-optimised.

3. **`firstpass.rs::scan_paragraph_interrupt_no_table`**. Called from
   several places in the block parser to decide whether a line breaks
   the current paragraph. Has 8 boolean/option args (suppressed
   `clippy::too_many_arguments`); inlines reasonably but is on the hot
   path for any document with paragraphs.

4. **`mdx_mark_and_unravel`**. Walks every paragraph node and inspects
   its children. Cheap per-node but runs unconditionally for MDX
   inputs. Could be skipped when the document has no MDX nodes at all
   — currently it always runs.

5. **`scan_mdx_jsx_block` and friends in `mdx.rs`**. Called for every
   line starting with `<` or `{` in MDX mode. Multi-line JSX scanning
   recurses through container-prefix stripping (`scan_containers`)
   for each continuation line.

### Code-quality candidates

- **`arena_build.rs` is 3.2k lines.** The parser-walk loop in `parse()`
  is the original `pulldown-cmark` event-driven structure ported to
  arena building, with a long `match` over `ItemBody`. Extracting the
  per-variant handlers would help comprehensibility but risks bloating
  the indirection.

- **`firstpass.rs::FirstPass::parse_block`** is the single biggest
  function in the parser (~1k lines). Branches by container kind, then
  by leaf-block kind. Hard to follow but the structure matches
  pulldown-cmark upstream.

- **MDX JSX attribute expression indent stripping in `mdx.rs`**. There
  are three nearly-parallel functions:
  `dedent_expression_continuation` (block expression body),
  `strip_expression_indent` (attribute expression), and
  `strip_attr_continuation_indent` (legacy, fewer columns). The math
  differs subtly per call site — some places use 2-column dedent + tab
  expansion, others use 1. This is the area I'm least confident about
  and the source of the remaining cloudflare divergence on
  `add-split-tunnels-route.mdx`.

- **`convert.rs::convert_node`** has grown long. It's a big match
  statement that emits hast for each mdast type. Footnote section
  emission and reference resolution are inline — could be helper
  functions without changing semantics.

### Allocation hot paths to spot-check

- `gfm_autolink_literal_pass` collects `Vec<(u32, bool)>` of candidate
  text node ids before splitting. The bool tracks "inside broken link
  label" — correct, but the vec allocation is unconditional even for
  documents with no candidates.

- `parse_mdx_jsx_flow` builds a `Vec<(usize, usize, usize)>` line map
  for stripped→original offset translation. Allocated per JSX flow
  block, which is fine, but worth confirming no flow blocks are tiny
  enough that a small-vec would beat heap.

- `collect_refs` allocates two `FxHashMap`s and a `Vec<u32>` for
  footnote tracking, plus a `FxHashMap<&str, u32>` for definition ids.
  The maps are short-lived (one per document) but every document with
  footnotes pays them.

## Testing strategy

Three layers, in order of strictness:

1. **Rust suite tests** (`crates/satteri-pulldown-cmark/tests/`).
   Auto-generated from `specs/*.txt` via `build.rs`; each test has a
   hardcoded HTML expected. These were originally inherited from
   pulldown-cmark / cmark-gfm and have been updated where the expected
   diverged from remark — every change-of-expected is annotated with a
   comment explaining why.

2. **JS conformance tests** (`packages/satteri/test/conformance/`).
   Each test runs the input through both satteri and remark (with the
   appropriate plugin set) and asserts deep equality on mdast/hast,
   or string equality on rendered HTML. `helpers.ts` exposes
   `assertMdastConformance`, `assertHastConformance`,
   `assertHtmlConformance`, plus extension-aware variants
   (`assertExtMdastConformance` for `frontmatter`/`directive`/`math`).
   All previously-fixed regressions have a test in `link-edge-cases`,
   `mdx-ast`, `directive`, or `spec-deltas`.

3. **Property fuzzers** (`packages/satteri/test/conformance/fuzz.test.ts`).
   `fast-check` generates random Markdown/GFM/MDX fragments and
   classifies any divergence by structural diff key. Issues are
   written to `FUZZ-ISSUES.md` / `FUZZ-ISSUES-MDX.md` for triage.
   Current state: 0 issues at 200 runs across all fuzz suites.

The reference pipeline for conformance is:

```
remark + remark-frontmatter + remark-gfm + remark-mdx
  + remark-directive + @mdx-js/mdx's remarkMarkAndUnravel
```

— configured per test file. JS conformance always uses `runSync` so
mark-and-unravel runs; pure-`parse` outputs are not the reference.

## Known limitations (intentionally not fixed in this branch)

### Cloudflare docs: 9 files diverge

8 of them have HTML-style tables with `<td>foo</td>   ` lines (≥2
trailing spaces = hard-break marker). Remark folds the entire run of
JSX-only lines into one paragraph because the hard-break links them;
we emit each line as its own JSX flow element. Properly fixing this
needs a lookahead pre-pass before the JSX flow classification: scan
all consecutive JSX-only lines until a blank line, and if any line
carries a hard-break, treat the whole run as paragraph-inline. We
documented this and decided not to ship it in this branch — the fix
risks regressing 8 simpler cases that an earlier attempt broke (per
the prior `KNOWN-DIVERGENCES.md`, since deleted along with the docs
corpora scripts).

The 9th file (`add-split-tunnels-route.mdx`) has deeply nested
`<Tabs><TabItem>` wrappers around a JSX attribute expression; the
continuation lines lose 1 tab level of indent. Fixing this needs
per-level wrapper-indent tracking through the strip-expression
machinery, which we don't currently model.

### MDX JS transform (`oxc_util_build_jsx.rs`)

Largest file in the diff (+1595) and the area I'm least familiar with.
Worth a focused pass — it's mostly oxc AST manipulation for
desugaring JSX into `_jsx`/`_jsxs`/`_Fragment` calls. The
`explicit_jsx` test in `crates/satteri-mdxjs-rs/tests/test.rs` was
updated to reflect that single-line `<h1>asd</h1>` stays a paragraph
(per remark) so the compiled output now wraps it in `_components.p`.

## Open questions / things I'd flag for myself

- The `scan_paragraph_interrupt_no_table` 8-arg signature is a code
  smell. A `ParseContext` struct grouping the booleans would be the
  obvious refactor; held off because the function is on a hot path
  and I didn't want to perturb inlining without measuring.

- `mdx_mark_and_unravel` runs unconditionally, even for non-MDX
  documents. Since the function returns early when no `Paragraph`
  nodes exist, the cost is bounded, but a "did parsing produce any
  MDX nodes?" gate at the top would skip it cleanly.

- We piggyback the GFM autolink-literal post-pass on
  `ENABLE_STRIKETHROUGH` because that flag historically meant "GFM
  features on". Now that `ENABLE_GFM` expands to include strikethrough
  at parse entry, the gate works for both, but the comment in
  `arena_build.rs` should probably just check `ENABLE_GFM` directly
  for clarity.

- The footnote-ordering change in `convert.rs` is correct for every
  test we have, but the BFS over referenced-definition bodies could
  in pathological cases re-visit the same definition many times. In
  practice docs have small footnote graphs — but worth a glance for a
  fixed-point guarantee.

## Where to start as a reviewer

If you have one hour:

1. Skim `arena_build.rs::parse` (lines ~39–~200 for control flow,
   then jump to the post-pass functions). This is where most of the
   "compose remark behaviour from pulldown-cmark + post-passes"
   intent lives.
2. Skim `convert.rs::collect_refs` and `convert_node`'s footnote arm.
3. Look at one of the bigger conformance tests in
   `packages/satteri/test/conformance/spec-deltas.test.ts` to see the
   format for verifying remark equivalence.

If you have half a day:

1. The above.
2. `firstpass.rs::parse_block` — particularly the JSX flow branches
   (~lines 660-700) and the task-list handling (~lines 200-280).
3. `mdx.rs` — focus on `scan_mdx_jsx_block`, `parse_mdx_jsx_flow`,
   and the three indent-stripping functions.
4. `oxc_util_build_jsx.rs` — at least skim, since it's a quarter of
   the diff.

For performance: profile against `packages/satteri/bench/` (the
existing bench harness; runs over fixture documents). If we want to
beat remark on real workloads, the post-passes are the first place to
look at fusing.
