# satteri-ast

## 0.1.4 — 2026-04-27

### Patch changes

- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Various MDX parsing fixes:
  
  - Fixed non-ASCII content in MDX expressions/JSX inside containers (blockquotes, lists) being corrupted due to byte-by-byte char casting.
  - Fixed MDX-only paragraphs inside blockquotes not being unraveled (producing spurious `<p>` wrappers).
  - Fixed multiple JSX elements on one line only rendering the first element.
  - Multiple other cases of small inconsistencies with `@mdxjs/mdx`, notably in whitespace handling and node positions. — Thanks @Princesseuh!
- [5736ca4](https://github.com/bruits/satteri/commit/5736ca45dd3eaf703e6d573f19274b42f1ca6cb9) Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans. — Thanks @Princesseuh!

## 0.1.3 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed script and style element contents being entity-escaped, which produced invalid output (e.g. `&lt;` inside `<script>`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed HAST property names not being mapped to their HTML attribute names during rendering (e.g. `className` now renders as `class`, `htmlFor` as `for`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed source positions being dropped for most node types during mdast-to-hast conversion, so hast plugins now see accurate positions across the tree — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed code blocks missing trailing newlines when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed footnote references and definitions not being rendered when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed table column alignment being dropped when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed `code.value` in the MDAST tree including a trailing newline for well-formed fenced code blocks, which diverged from `remark-parse`. MDAST plugins inspecting `node.value` now see the same bytes as remark. — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed task list classes and checkbox inputs being missing when using hast plugins — Thanks @Princesseuh!

## 0.1.2 — 2026-04-14

### Patch changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!
- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side. — Thanks @Princesseuh!

