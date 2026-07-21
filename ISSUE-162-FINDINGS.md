# Issue #162 — `headingAttributes` errors in MDX

> **Investigation only.** No fix applied. Findings + options below.

## Summary

Custom heading IDs via the `headingAttributes` syntax (`# Heading {#custom-id}`)
work in `.md` but throw in `.mdx`:

```
1:23: Could not parse expression with oxc: Expected `in` but found `-` (mdx-jsx:unexpected-character)
```

This is **not an accidental bug** — satteri deliberately disables heading
attributes whenever MDX is enabled, so `{...}` is treated as an MDX expression.
The error is oxc rejecting `#custom-id` as invalid JavaScript. This matches the
behavior and the design stance of the entire unified/MDX ecosystem, which treats
this exact collision as by-design and out of scope.

## Reproduced locally

`# Heading {#custom-id}\n`:

| Options                                        | Result                                                                            |
| ---------------------------------------------- | --------------------------------------------------------------------------------- |
| `DEFAULT_OPTIONS \| ENABLE_HEADING_ATTRIBUTES` | ✅ no errors, id applied                                                          |
| `MDX_OPTIONS \| ENABLE_HEADING_ATTRIBUTES`     | ❌ `(18, "Could not parse expression with oxc: Expected \`in\` but found \`-\`")` |

Offset 18 is the `-` in `custom-id` — exactly the reported error.

## Root cause (satteri)

satteri is a **single-pass Rust tokenizer** (`crates/satteri-pulldown-cmark`).
There is no separate "run remark transform plugins on a finished AST" phase — the
`{...}` bytes are claimed during the first pass and validated by oxc _inline_.

Two places gate heading-attribute parsing off under MDX:

- **ATX headings** — `firstpass.rs:3207-3208`:
  ```rust
  let (end, content_end, attrs) = if self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES)
      && !self.options.contains(Options::ENABLE_MDX)   // <-- disabled under MDX
  ```
- **Setext + shared helper** — `extract_and_parse_heading_attribute_block`,
  `firstpass.rs:3719-3723`:
  ```rust
  if !self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES)
      || self.options.contains(Options::ENABLE_MDX)    // <-- early return None under MDX
  {
      return None;
  }
  ```

The accompanying comment (`firstpass.rs:3204-3206`) states the intent:

> When MDX is enabled, `{...}` in headings should be treated as MDX expressions,
> not heading attribute blocks. MDX expressions and heading attributes use the
> same `{...}` syntax and would conflict.

So under MDX, the trailing `{#custom-id}` falls through to the inline MDX
expression path (`firstpass.rs:2051+`). There, the braced body `#custom-id` is
handed to oxc via `crate::mdx::try_parse_expression_body`
(`firstpass.rs:2113-2117`). oxc reads `#custom` as a **private identifier**,
which in JS is only legal in the "ergonomic brand check" form `#priv in obj`; it
expects `in`, finds `-`, and errors. That error is pushed onto `mdx_errors`.

### How a user hits it

The napi binding (`crates/satteri-napi-binding/src/lib.rs:127-159`) lets JS set
`headingAttributes: true` and `mdx: true` independently. When both are on, the
firstpass keeps the `ENABLE_MDX` branch, so `headingAttributes` is silently
inert and the syntax errors instead. Starlight/Astro users enabling heading
attributes for their `.md` content hit this the moment a page is `.mdx`.

Docs gap: `website/content/docs/features.md:128-176` documents heading
attributes with no mention that they don't apply in MDX.

## How the unified/MDX ecosystem handles this exact collision

The ecosystem answer is unambiguous and directly analogous to satteri's design.

**The collision is real, fundamental, and by-design.** In MDX, `{...}` is claimed
as a JS expression at the **micromark tokenizer (syntax) level**, which runs
_before_ any mdast/remark transform plugin sees the tree. Pipeline order:

1. **micromark tokenization** — `micromark-extension-mdx-expression` registers a
   construct on `{` (code 123), emits `mdxFlowExpression`/`mdxTextExpression`
   tokens, and **runs acorn on the braced content right here**. This is where
   `Could not parse expression with acorn` is thrown — at tokenize time.
2. mdast construction (`mdast-util-mdx-expression`).
3. **remark transform plugins run** — on the finished tree.
4. rehype/HAST → output.

**Consequence (Q1 + Q2):** AST-visitor heading-id plugins can't work in MDX.
`remark-heading-id`, `remark-attr`-style plugins run at step 3, after `{#id}` has
already been tokenized (or errored) at step 1. This is a documented **v1→v2
regression**: MDX v1 kept `A heading {#custom-id}` as one text node a visitor
could regex; v2 splits it into `text` + `mdxTextExpression` (or errors).
Maintainers close these `wontfix`:

- wooorm (mdx-js/mdx #1279): _"it's by design: `{...}` is an expression here…
  `{1 + 1}` could output 2."_
- ChristianMurphy (#1953): _"`unist-util-visit` works on the AST, after the
  document has already parsed, which means JSX parsing will always take
  precedence… If you want to add a new syntax feature, it needs to be
  implemented as part of the parser."_
- remcohaszing (#2485): _"what's between `{` and `}` needs to be valid
  JavaScript… `#custom-id` isn't valid JavaScript, so this is a parsing error."_

**Workarounds the ecosystem actually recommends (Q3):**

1. **`rehype-slug`** — auto-generates IDs from heading _text_ at the HAST stage,
   after MDX expressions resolve. Sidesteps the collision entirely. The top
   maintainer recommendation. Trade-off: you don't choose the exact id string,
   and it can't slug a dynamic `# {name}`.
2. **Raw JSX in the heading** — `<h1 id="custom-id">A heading</h1>`. Native MDX,
   always works.
3. **Escape the braces** — `# title \{#custom-anchor\}`. Stops MDX claiming `{`,
   keeps `{#id}` as literal text a plugin can read. "Ugly but works with MDX."
4. **Non-brace delimiter at HAST stage** — `rehype-slug-custom-id` supports
   `# Heading [#custom-slug]` (square brackets dodge MDX). Runs after `rehype-slug`.
5. **A genuine micromark syntax extension** (`Eyas/md-heading-id`) that competes
   for `{` at step 1. This is the _only_ path that reads `{#id}` in MDX — but
   composing two `{` constructs is fragile: users report it "breaks regardless of
   which plugin comes first" (Eyas/md-heading-id #2), and core maintainers won't
   support forking MDX's brace grammar.

**Precedence between two `{` micromark extensions (Q4):** `combineExtensions`
collects all constructs registered for a code into a list; new ones **prepend**
by default (`add: 'after'` to append); micromark tries them in list order with
backtracking, first success wins. So a `{#id}`-that-fails-fast construct _could_
sit ahead of MDX's expression construct — but users don't control MDX's internal
registration order relative to their plugin, and MDX runs acorn inside its
construct, so the interplay is unreliable in practice.

## What this means for satteri

satteri's current behavior (disable heading attrs under MDX, let `{...}` be an
expression) is **exactly consistent with mdx-js's design stance**. It is
defensible and matches user expectations coming from the unified world.

The key structural difference in satteri's favor: **satteri owns its entire
tokenizer.** Unified users can't reliably compose a `{#id}` construct with MDX's
`{` construct because they don't control MDX's extension registration. satteri
_is_ both extensions, in one pass, so it can special-case the collision
deterministically if it chooses to — no backtracking-precedence lottery.

Note also `#custom-id` (and `.class`, `key=value`, bare `key`) is **never valid
JavaScript**, so a targeted heading-attribute-shaped `{...}` at the end of a
heading cannot also be a legitimate MDX expression. That makes disambiguation
low-risk: the only `{...}` that a heading-attribute reader would claim are ones
oxc would reject anyway.

### Options

**A. Do nothing but document it (lowest effort).**
Update `features.md` to state heading attributes don't apply in MDX, and point
MDX users at the alternatives below. Matches ecosystem convention. Leaves the
current confusing hard _error_ in place (arguably worse than unified, which at
least fails the same way — but satteri could soften the error; see B).

**B. Special-case heading-attribute-shaped trailing `{...}` in headings under MDX.**
In the ATX/setext heading paths, before handing a trailing `{...}` to oxc, test
whether it parses as a heading attribute block (`parse_inside_attribute_block`,
`firstpass.rs:6716`). If it does _and_ it sits at heading-end, consume it as
heading attributes instead of an MDX expression. Because attribute-block bodies
are never valid JS, this doesn't steal any legitimate expression. This is the
"true syntax extension" path — but satteri can do it cleanly since it owns the
tokenizer. Risk: edge cases where a heading legitimately ends in a valid JS
expression that also happens to look attribute-like (rare; `{.x}` / `{#x}` /
`{k=v}` shapes don't collide with real expressions). Needs care with the
existing directive-run search-end logic (`heading_attr_block_search_end`).

**C. HAST/plugin-stage IDs (the `rehype-slug` analog).**
satteri has a post-parse plugin API with typed `Heading` nodes
(`crates/satteri-plugin-api/src/typed_nodes.rs:29`). A slug plugin could derive
`id` from heading text after MDX expressions resolve — works uniformly for `.md`
and `.mdx`, no brace collision. Doesn't honor an _explicit_ chosen id, and can't
slug a dynamic heading, same as `rehype-slug`. Could ship as a bundled plugin or
documented recipe.

**D. Support an escape or non-brace syntax for MDX** (`\{#id\}`, or `[#id]`).
Matches ecosystem workarounds 3/4. More surface area / another syntax to teach;
probably inferior to B if we're willing to own the tokenizer special-case.

### Recommendation

Lead with **B** (special-case the attribute-shaped trailing block in headings
under MDX) — it's the behavior users clearly want, it's safe precisely because
attribute bodies are never valid JS, and satteri is uniquely positioned to do it
where unified can't. Pair it with a docs note. If B is deemed too much parser
surface for now, ship **A + C**: document the limitation and offer a slug plugin
as the supported path, exactly as the unified ecosystem does. Either way, at
minimum stop silently erroring — today `headingAttributes: true` + `mdx: true`
is a foot-gun with no in-product signal.

## Key references

- satteri: `firstpass.rs:3204-3223` (ATX gate), `:3719-3723` (helper gate),
  `:2051-2140` (inline MDX expr + oxc validation), `:6716` (attr-block parser);
  `satteri-napi-binding/src/lib.rs:127-159` (option mapping);
  `website/content/docs/features.md:128-176` (docs).
- mdx-js/mdx #1279, #1953, #2485 (by-design, parser-stage, `wontfix`).
- imcuttle/remark-heading-id #9 (`\{#id\}` escape); Eyas/md-heading-id #2
  (micromark extension, fragile composition).
- `rehype-slug`, `rehype-slug-custom-id` (HAST-stage IDs; `[#slug]` notation).
- `micromark-extension-mdx-expression`, `mdast-util-mdx-expression`,
  `micromark-util-combine-extensions` (tokenizer claims `{`; construct precedence).
