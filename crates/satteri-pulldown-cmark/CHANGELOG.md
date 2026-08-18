# satteri-pulldown-cmark

## 0.6.0 — 2026-08-18

### Minor changes

- [eeb7f07](https://github.com/bruits/satteri/commit/eeb7f0778a7af229fd592dd027ddfe0723ba2b26) Faster parsing, MDX compilation, and plugin execution. — Thanks @Princesseuh!
- [d8b7172](https://github.com/bruits/satteri/commit/d8b71724ba3a6bfcad24265c5b1d021b1de1eaa0) Adds a `definitionList` feature (off by default) that renders definition lists to `<dl>`/`<dt>`/`<dd>`.
  
  New `descriptionList` / `descriptionTerm` / `descriptionDetails` nodes are available to plugins when this option is enabled.
  
  ```text
  Apple
  :   Pomaceous fruit.
  :   A tech company.
  ```
   — Thanks @lolifamily for your first contribution 🎉!
- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Faster across the board: parsing is ~10% cheaper, editing the tree from plugins now costs proportionally to how much you change rather than how big the document is (3 edits on a 115KB document: ~160µs → under 50µs), reading nodes inside plugins is 40-75% faster, and memory stays flat under sustained workloads. — Thanks @Princesseuh!

### Patch changes

- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a line holding only a vertical tab or form feed counting as a blank line, which split paragraphs and let a definition run past its destination. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks sometimes missing position information, such as a bare URL after an unclosed `[`. — Thanks @Princesseuh!
- [c9985d9](https://github.com/bruits/satteri/commit/c9985d93b5ee23aff07491360be83d4a3412f18b) Fixed `development: true` line and column numbers, missing-component references, and MDX parse error locations being wrong in documents with multibyte or emoji characters. — Thanks @Princesseuh!
- [7441ecd](https://github.com/bruits/satteri/commit/7441ecd029d800c567d5c5c9d102bd0bfc0a9e9e) Fixed a defined footnote reference like `[^x](y)` parsing as a link instead of a footnote reference followed by text. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX expressions failing to parse when a string inside them is continued over a CRLF line ending with a backslash. — Thanks @Princesseuh!
- [0d26ea6](https://github.com/bruits/satteri/commit/0d26ea6d68a29d4de8419423e030076244348c22) Changed the minimum supported Rust version to 1.85, as these crates now build on the 2024 edition. — Thanks @Princesseuh!
- [ac99c4f](https://github.com/bruits/satteri/commit/ac99c4f9ecf4e2fa3b5eb1dbf069160f1ba7a6f1) Improved parsing performance for documents with few or no autolink candidates. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when a character reference supplies its first character, as in `&#104;ttp://example.com`. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX line comments and `import`/`export` blocks swallowing the lines after them in files that use lone carriage returns as line endings. — Thanks @Princesseuh!
- [d6dbbad](https://github.com/bruits/satteri/commit/d6dbbad1d47e43f10391b3e00792078da49bdfc7) Fixed an email overlapping a `www.` link swallowing the link, like `user@www.example.org` after an unclosed bracket. — Thanks @Princesseuh!
- [be2c1a1](https://github.com/bruits/satteri/commit/be2c1a168fdcc548b0c39980a3e4be1634acae8d) Fixed emphasis being parsed around a `~` when GFM is disabled, so `a*~*` now stays plain text. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed GFM autolinks losing their positions when smart punctuation is enabled. — Thanks @Princesseuh!
- [18d123b](https://github.com/bruits/satteri/commit/18d123bb749d4f6fb0fca4fc1e79129761958873) Fixed a `{` inside an MDX link destination or title raising a parse error when the link tail spans more than one line, as in `[a](/u\n"ti{tle")`. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed at the end of a line being dropped from the text. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed inline code ending at the wrong backtick when its content looks like a URL. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed emphasis and character references being lost after a GFM autolink whose URL ends in a backslash. — Thanks @Princesseuh!
- [1126ad0](https://github.com/bruits/satteri/commit/1126ad0dc303de1f3f3eeccfb8355bd0b99d2eb9) Fixed a tight definition list gluing a definition's continuation paragraph onto the first block with no separator. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed standing in for a space in an ATX heading opener, a task list marker, an HTML tag, or a link or footnote label. — Thanks @Princesseuh!
- [204fb3a](https://github.com/bruits/satteri/commit/204fb3aac413201e6a99bc0bfc54c4e8d199d425) Fixed documents with many unclosed parenthesized link titles taking quadratic time to parse. — Thanks @Princesseuh!
- [2e3ed23](https://github.com/bruits/satteri/commit/2e3ed23aa0e2489c4ce667cb39eb29259664692d) Faster Markdown-to-HTML rendering, most noticeably on prose-heavy documents where GFM autolink scanning dominated: a 200KB CommonMark document renders about 7% faster end to end. — Thanks @Princesseuh!
- [64f3d5f](https://github.com/bruits/satteri/commit/64f3d5f8666851494195ebd150bfa47df4da56e9) Fixes inline code being mangled when it contains directive-like syntax. With directives enabled, writing something like `` `:foo[` `` followed by more inline code no longer merges the two code spans or drops a backtick: a `:` inside a code span is now treated as literal text, so you can safely show directive syntax in code. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a `www.` URL linking when the character right before it is U+0085, which does not separate words. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks getting the wrong URL, or being dropped entirely, when a `[` earlier in the paragraph belongs to a code span, inline HTML, a pointed autolink, or a link that never resolves. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed text being dropped when an MDX expression contains the `]` that ends a reference label. — Thanks @Princesseuh!
- [7e28d6c](https://github.com/bruits/satteri/commit/7e28d6cd1251b92e337a6ab57b75aa55d923fad2) Fixed a `:directive` after an invalid bare URL being destroyed instead of parsed, as in `http://my_app.localhost:3000/admin`. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved parsing performance for documents with many link reference definitions inside lists or blockquotes. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed a hard line break inside an image label adding a stray newline to the image's alt text. — Thanks @Princesseuh!
- [46e2572](https://github.com/bruits/satteri/commit/46e25721656ec01fe494b62a3c2a5a48f1e45dfb) Fixed a `{` inside an MDX link destination or title raising a parse error when the tail holds an escaped or quoted `)`, as in `[a](\){)`, and stopped a link tail forming from a `[` that is backslash-escaped, inside a code span, already wrapped by another link, or in an earlier block. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved parsing performance for documents with paragraphs inside lists, blockquotes, and other containers. — Thanks @Princesseuh!
- [2c14a38](https://github.com/bruits/satteri/commit/2c14a38e56d4903ccc2e933bb74c63d4c1426147) Fixed links and reference definitions whose parenthesized title holds an unescaped `(`, as in `[a](* (())`, not being parsed as links, and in MDX a `{` inside such a title no longer raises a parse error. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed two bare URLs separated by a `]` being merged into one over-long link, as in `[www.a.com]www.b.com`. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL swallowing the text after it when the two are separated by a non-breaking space or other Unicode whitespace. — Thanks @Princesseuh!
- [acee492](https://github.com/bruits/satteri/commit/acee492ddc0e703eaaed5169f52f7e7c7cf971ac) Fixed a link title being accepted with no whitespace after a `<...>` destination, so `[a](<u>"t")` is now plain text like in remark. — Thanks @Princesseuh!
- [6696c1c](https://github.com/bruits/satteri/commit/6696c1c28b3024c5c8df760cc5af51dd713663fc) Fixed `position` offsets being wrong in documents with multibyte characters. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed documents that use standalone carriage returns (`\r`) as line endings parsing differently from documents that use `\n`. Values such as inline code and definition titles now keep the document's own line endings instead of always reporting `\n`. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed the start offset of text in a table cell when it begins with an escaped pipe. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed an email address starting with `www.` linking as a URL instead of an email. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed the text after a GFM autolink being mangled when the URL ends on a character reference or a backslash, which could decode the wrong character, report an overlapping position, or swallow the inline HTML or emphasis that followed. — Thanks @Princesseuh!
- [50824f3](https://github.com/bruits/satteri/commit/50824f3dfbd8b67a2aaac0b643725fa9e3b624ba) Fixed every position being shifted by one in documents that start with a byte-order mark. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed fenced code block `lang` and `meta` splitting on whitespace that a character reference produced. — Thanks @Princesseuh!
- [419e711](https://github.com/bruits/satteri/commit/419e711fd4e3092c84fff462d3bbbae406a09472) With smart punctuation enabled, an unmatched close-flanking double quote (like the inch mark in `24" monitor`) now renders as a closing curly quote instead of an opening one. A double quote after a digit no longer opens a quotation, so dimension notation like `24"x36"` closes throughout. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when the character just before it is an emoji or other astral punctuation. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.3.0, satteri-ast (Cargo)@0.5.0

## 0.5.8 — 2026-07-08

### Patch changes

- [d2c33ca](https://github.com/bruits/satteri/commit/d2c33ca65721a45b2899a5265d54a226a3843a91) Fixed emphasis, strikethrough, and subscript/superscript sometimes nesting in the wrong order. — Thanks @Princesseuh!
- [d2c33ca](https://github.com/bruits/satteri/commit/d2c33ca65721a45b2899a5265d54a226a3843a91) Fixed URLs inside angle brackets like `<https://www.example.com>` mangling the punctuation and line break that immediately followed them. — Thanks @Princesseuh!
- [24a4689](https://github.com/bruits/satteri/commit/24a4689c670d7752cd6fc1ecc0f866d57f034924) Fixed heading attribute blocks conflicting with text directives when both features are enabled. — Thanks @Princesseuh!
- [d2c33ca](https://github.com/bruits/satteri/commit/d2c33ca65721a45b2899a5265d54a226a3843a91) Improved automatic linking of bare URLs and emails to match GitHub more closely, including uppercase schemes like `HTTP://` and `WWW.`, `www` hosts without a second dot, trailing punctuation, and not linking inside existing link text. — Thanks @Princesseuh!
- [d2c33ca](https://github.com/bruits/satteri/commit/d2c33ca65721a45b2899a5265d54a226a3843a91) Fixed a performance issue when parsing documents with many links in certain contexts — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.4.2

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

