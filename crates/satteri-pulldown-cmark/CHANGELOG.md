# satteri-pulldown-cmark

## 0.5.7 — 2026-06-29

### Patch changes

- [07ee532](https://github.com/bruits/satteri/commit/07ee53293af76d0dcddbac961ad35337c5500e74) Fixes JSX nested in an MDX attribute expression (e.g. `prop={<p>hi</p>}` or `title={<>x</>}`) being emitted as raw, un-lowered JSX, which produced invalid JavaScript. Also fixes quotes and apostrophes in such JSX text (e.g. `prop={<p>Acme Corp.'s "best" tool</p>}`) being mis-scanned as JS string literals and causing a parse error — the expression scanner now consumes a JSX element's children as text. — Thanks @vaneenige for your first contribution 🎉!
- Updated dependencies: satteri-arena (Cargo)@0.2.2, satteri-ast (Cargo)@0.4.1

## 0.5.6 — 2026-06-25

### Patch changes

- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes a blank line inside a template literal or block comment in an MDX `import`/`export` causing an `Unterminated string` error. The blank line no longer ends the statement early. — Thanks @Princesseuh!
- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes inline math like `$\frac{-b}{2a}$` failing to compile in MDX. Braces inside `$...$` are now treated as math text, not a JSX expression. — Thanks @Princesseuh!
- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes quotes inside a regex in an MDX JSX attribute (e.g. `ins={[/icon="[^"]+"/g]}`) causing a parse error. — Thanks @Princesseuh!
- [27c9023](https://github.com/bruits/satteri/commit/27c90239935f218103995a4d82a6473dc1d728f8) Fixes `headingAttributes` silently dropping parsed attributes. — Thanks @Princesseuh!

## 0.5.5 — 2026-06-19

### Patch changes

- [855379c](https://github.com/bruits/satteri/commit/855379c7eb018e9c5acc69daa7a63f27dbb79e7f) Fix MDX `import`/`export` blocks being broken by a following whitespace-only line. A line containing only spaces or tabs now ends the ESM block exactly like an empty line, instead of being consumed as a statement continuation (which produced a `Could not parse esm with oxc` error). — Thanks @Princesseuh!
- [855379c](https://github.com/bruits/satteri/commit/855379c7eb018e9c5acc69daa7a63f27dbb79e7f) MDX parse errors now carry a source line and column. Previously, errors in `import`/`export` blocks dropped the position entirely, and errors in `{…}` expressions and JSX attributes were reported as a bare byte offset, so downstream tooling reported an unknown location. JSX attribute and spread expression errors now point at the offending attribute rather than the element's opening `<`. — Thanks @Princesseuh!

## 0.5.4 — 2026-06-18

### Patch changes

- [6bcdf06](https://github.com/bruits/satteri/commit/6bcdf06a0ee267779180a2d89a27a31f2f4b5b81) `features.superscript` and `features.subscript` now render `^text^` as `<sup>text</sup>` and `~text~` as `<sub>text</sub>` as documented, instead of `<em>`. The MDAST now exposes dedicated `superscript` and `subscript` node types, which plugins can visit and construct. Plugins that previously matched these spans as `emphasis` nodes should switch to the new node types. — Thanks @morinokami for your first contribution 🎉!
- Updated dependencies: satteri-ast (Cargo)@0.4.0

## 0.5.3 — 2026-06-11

### Patch changes

- [42835bc](https://github.com/bruits/satteri/commit/42835bcad387064678421d5623067500c4cefa1c) Fixes a smart punctuation issue where double quotes could be rendered with the wrong direction when quoted text appeared next to text without whitespace. — Thanks @HiDeoo for your first contribution 🎉!

## 0.5.2 — 2026-06-08

### Patch changes

- [e58b500](https://github.com/bruits/satteri/commit/e58b500aecfce9c03e3a5045a2d5a063eb1f8203) Fixes a parsing error when a MDX attribute contained the closing tag of itself, e.g. `<Component attr="</Component>">`. The parser would incorrectly treat the `</Component>` as the closing tag of the component, instead of part of the attribute value. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.3.2

## 0.5.1 — 2026-06-03

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.1

## 0.5.0 — 2026-06-02

### Minor changes

- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.math` control. `singleDollarTextMath: false` keeps single-`$` constructs as literal text (so prose can carry currency like "$50 to $100") while `$$ ... $$` still parses as display math.
  
  ```ts
  markdownToHtml(source, {
    features: { math: { singleDollarTextMath: false } },
  });
  ```
   — Thanks @Princesseuh!
- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Directive labels now render full Markdown. `:::note[Custom **strong** Label]` shows bold text instead of literal `**` markers. Emphasis, links, inline code, and (in MDX) components and expressions all work inside a label now, on container, leaf, and text directives. Previously a label only understood inline code.
  
  Directives that end with an HTML block also close cleanly now. A `:::note` whose last line before the closing fence is `</details>` no longer leaks a stray `:::` into the output. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.2.1, satteri-ast (Cargo)@0.3.0

## 0.4.1 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.0, satteri-ast (Cargo)@0.2.7

## 0.4.0 — 2026-05-18

### Minor changes

- [e8f7974](https://github.com/bruits/satteri/commit/e8f7974149d5a6f40391520059b174cae5665ff2) Fix borked publish — Thanks @Princesseuh!

## 0.3.6 — 2026-05-12

### Patch changes

- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed plugin-inserted MDX JSX elements compiling as literal HTML tags instead of routing through `_components`, which prevented user overrides via the `components` prop. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.6

## 0.3.5 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4, satteri-ast (Cargo)@0.2.5

## 0.3.4 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4

## 0.3.3 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.3

## 0.3.2 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.2

## 0.3.1 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.1

## 0.3.0 — 2026-04-29

### Minor changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Renamed `Options::ENABLE_CONTAINER_EXTENSIONS` to `Options::ENABLE_DIRECTIVE`. If you use this crate directly, update the option name; if you only consume satteri through the npm package or the high-level Rust API, no change is needed (the `features.directive` toggle keeps its name). — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.0

## 0.2.5 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3, satteri-ast (Cargo)@0.1.5

## 0.2.4 — 2026-04-27

### Patch changes

- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Various MDX parsing fixes:
  
  - Fixed non-ASCII content in MDX expressions/JSX inside containers (blockquotes, lists) being corrupted due to byte-by-byte char casting.
  - Fixed MDX-only paragraphs inside blockquotes not being unraveled (producing spurious `<p>` wrappers).
  - Fixed multiple JSX elements on one line only rendering the first element.
  - Multiple other cases of small inconsistencies with `@mdxjs/mdx`, notably in whitespace handling and node positions. — Thanks @Princesseuh!
- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Added granular smart punctuation options (`ENABLE_SMART_QUOTES`, `ENABLE_SMART_DASHES`, `ENABLE_SMART_ELLIPSES`) that can be enabled independently instead of the entire group. — Thanks @Princesseuh!
- [5736ca4](https://github.com/bruits/satteri/commit/5736ca45dd3eaf703e6d573f19274b42f1ca6cb9) Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.1.4

## 0.2.3 — 2026-04-17

### Patch changes

- [11ffcfc](https://github.com/bruits/satteri/commit/11ffcfca6c8486a3744e37e0c19e78100925323e) Fixed unclosed `{` in a paragraph silently consuming later blocks as an MDX expression, and fixed literal `{` inside code spans being falsely reported as an unclosed MDX expression — Thanks @Princesseuh!

## 0.2.2 — 2026-04-16

### Patch changes

- [6f9f66f](https://github.com/bruits/satteri/commit/6f9f66fa75722c0b58f50783b5ac85fefd53a157) Fixed JSX inside MDX expression bodies, JSX inside `.map()` callbacks or other expressions is now compiled to `_jsx()` calls instead of being dropped or emitted as raw JSX — Thanks @Princesseuh!

## 0.2.1 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed `code.value` in the MDAST tree including a trailing newline for well-formed fenced code blocks, which diverged from `remark-parse`. MDAST plugins inspecting `node.value` now see the same bytes as remark. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.1.3

## 0.2.0 — 2026-04-14

### Minor changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2

