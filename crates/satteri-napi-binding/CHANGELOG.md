# satteri-napi

## 0.5.0 — 2026-08-18

### Minor changes

- [d8b7172](https://github.com/bruits/satteri/commit/d8b71724ba3a6bfcad24265c5b1d021b1de1eaa0) Adds a `definitionList` feature (off by default) that renders definition lists to `<dl>`/`<dt>`/`<dd>`.
  
  New `descriptionList` / `descriptionTerm` / `descriptionDetails` nodes are available to plugins when this option is enabled.
  
  ```text
  Apple
  :   Pomaceous fruit.
  :   A tech company.
  ```
   — Thanks @lolifamily for your first contribution 🎉!
- [d8639d6](https://github.com/bruits/satteri/commit/d8639d64efa50f2adf2f88f6a4928559d2a30836) Added a `rawHtml` feature that reparses raw HTML embedded in Markdown into real HAST nodes. Enable it with `features: { rawHtml: true }` on any entry point; it is applied during the MDAST→HAST conversion, so `markdownToHast`, `markdownToHtml`, and the plugin pipelines all reparse identically, and hast plugins always see the reparsed elements.
  
  The whole tree is reparsed through the HTML parser, so a tag opened in one raw block and closed in another is resolved against the surrounding Markdown. Attributes are normalized into typed hast properties (`class` → `className: [...]`, `disabled` → `true`, `tabindex` → number, `data-foo-bar` → `dataFooBar`). `htmlToHast` normalizes properties the same way.
  
  MDX nodes are passed through the reparse rather than dropped: each JSX element/expression is preserved in place while the surrounding raw HTML is still resolved around it. So `mdxToHast(source, { features: { rawHtml: true } })` keeps its MDX content.
  
  ```ts
  import { markdownToHast } from "satteri";
  
  const tree = markdownToHast(`<div class="note">\n\n**hi**\n\n</div>`, {
    features: { rawHtml: true },
  });
  // <div> is a real element wrapping <p><strong>hi</strong></p>
  ```
   — Thanks @IEvangelist for your first contribution 🎉!
- [166419c](https://github.com/bruits/satteri/commit/166419cf912b3639abedfcb87ee8059920e5b221) Added `markdownToJs`, the plain-Markdown counterpart to `mdxToJs`: MDX syntax like `{...}` stays literal text.
  
  ```ts
  import { markdownToJs } from "satteri";
  
  const { code } = markdownToJs("Hello {world}");
  ```
  
  HTML in the source is dropped. Pass `features: { rawHtml: true }` to parse it into real elements instead. — Thanks @Princesseuh!
- [2ac113e](https://github.com/bruits/satteri/commit/2ac113e9851dfd15340a999f9a1e829a9d2b0f8f) Added `position: false` to `markdownToMdast`, `mdxToMdast`, `markdownToHast`, and `mdxToHast`, which skips recording `node.position`. On a 1 MB document that halves both the time to build a tree and the memory it occupies, so it is worth passing whenever nothing downstream reads positions.
  
  ```ts
  const tree = markdownToMdast(source, { position: false });
  ```
   — Thanks @Princesseuh!
- [d8639d6](https://github.com/bruits/satteri/commit/d8639d64efa50f2adf2f88f6a4928559d2a30836) Added `htmlToHast`, which parses an HTML string into a HAST tree (elements, text, comments, doctype) with the same spec-compliant parsing a browser does. The result is a `root` wrapping the implied `<html>` subtree.
  
  ```ts
  import { htmlToHast } from "satteri";
  
  const tree = htmlToHast("<p>hi</p>");
  // { type: "root", children: [{ type: "element", tagName: "html", ... }] }
  ```
   — Thanks @IEvangelist for your first contribution 🎉!
- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Faster across the board: parsing is ~10% cheaper, editing the tree from plugins now costs proportionally to how much you change rather than how big the document is (3 edits on a 115KB document: ~160µs → under 50µs), reading nodes inside plugins is 40-75% faster, and memory stays flat under sustained workloads. — Thanks @Princesseuh!

### Patch changes

- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a line holding only a vertical tab or form feed counting as a blank line, which split paragraphs and let a definition run past its destination. — Thanks @Princesseuh!
- [ac99c4f](https://github.com/bruits/satteri/commit/ac99c4f9ecf4e2fa3b5eb1dbf069160f1ba7a6f1) Improved Markdown to HTML performance when no HAST plugins run and nothing sets `hName`, `hProperties`, or `hChildren` (which heading attributes do). — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks sometimes missing position information, such as a bare URL after an unclosed `[`. — Thanks @Princesseuh!
- [c9985d9](https://github.com/bruits/satteri/commit/c9985d93b5ee23aff07491360be83d4a3412f18b) Fixed `development: true` line and column numbers, missing-component references, and MDX parse error locations being wrong in documents with multibyte or emoji characters. — Thanks @Princesseuh!
- [f868e26](https://github.com/bruits/satteri/commit/f868e26e8c07a5e30b90b16b554835f73f37d0c0) Fixed React-cased SVG property names like `strokeLinecap` and `strokeLinejoin` leaking into HTML output as-is instead of serializing as `stroke-linecap` / `stroke-linejoin`. — Thanks @gtritchie!
- [7441ecd](https://github.com/bruits/satteri/commit/7441ecd029d800c567d5c5c9d102bd0bfc0a9e9e) Fixed a defined footnote reference like `[^x](y)` parsing as a link instead of a footnote reference followed by text. — Thanks @Princesseuh!
- [7e9ac4c](https://github.com/bruits/satteri/commit/7e9ac4c38b7cd4ede2eaf4353765d74e905e45ba) Fixed very deeply nested documents crashing the process instead of compiling. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX expressions failing to parse when a string inside them is continued over a CRLF line ending with a backslash. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when a character reference supplies its first character, as in `&#104;ttp://example.com`. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX line comments and `import`/`export` blocks swallowing the lines after them in files that use lone carriage returns as line endings. — Thanks @Princesseuh!
- [d6dbbad](https://github.com/bruits/satteri/commit/d6dbbad1d47e43f10391b3e00792078da49bdfc7) Fixed an email overlapping a `www.` link swallowing the link, like `user@www.example.org` after an unclosed bracket. — Thanks @Princesseuh!
- [be2c1a1](https://github.com/bruits/satteri/commit/be2c1a168fdcc548b0c39980a3e4be1634acae8d) Fixed emphasis being parsed around a `~` when GFM is disabled, so `a*~*` now stays plain text. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed GFM autolinks losing their positions when smart punctuation is enabled. — Thanks @Princesseuh!
- [18d123b](https://github.com/bruits/satteri/commit/18d123bb749d4f6fb0fca4fc1e79129761958873) Fixed a `{` inside an MDX link destination or title raising a parse error when the link tail spans more than one line, as in `[a](/u\n"ti{tle")`. — Thanks @Princesseuh!
- [2b85f56](https://github.com/bruits/satteri/commit/2b85f5602fc3340eef9faa3e41c66ff0a03ec8af) Adds `{ raw }` support to `wrapNode()` in HAST plugins: the HTML is parsed and the node is wrapped in the resulting element. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed at the end of a line being dropped from the text. — Thanks @Princesseuh!
- [5c4cd17](https://github.com/bruits/satteri/commit/5c4cd170b2e4d0db4fb9f610fc15802aa2757fd9) Fixed `elementAttributeNameCase: "html"` leaving a nested `<svg>` element's own React-cased attributes (like `strokeWidth`) unconverted on the MDX compile path; the SVG schema now covers the `<svg>` element itself, not just its descendants. — Thanks @gtritchie!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed inline code ending at the wrong backtick when its content looks like a URL. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed emphasis and character references being lost after a GFM autolink whose URL ends in a backslash. — Thanks @Princesseuh!
- [1126ad0](https://github.com/bruits/satteri/commit/1126ad0dc303de1f3f3eeccfb8355bd0b99d2eb9) Fixed a tight definition list gluing a definition's continuation paragraph onto the first block with no separator. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed standing in for a space in an ATX heading opener, a task list marker, an HTML tag, or a link or footnote label. — Thanks @Princesseuh!
- [6a1eaec](https://github.com/bruits/satteri/commit/6a1eaecb25e442d26bc6ee90ac63bdd28c4bd465) Fixed `wrapNode()` not accepting the `{ raw }` shape that every other structural mutator takes. — Thanks @Princesseuh!
- [204fb3a](https://github.com/bruits/satteri/commit/204fb3aac413201e6a99bc0bfc54c4e8d199d425) Fixed documents with many unclosed parenthesized link titles taking quadratic time to parse. — Thanks @Princesseuh!
- [166419c](https://github.com/bruits/satteri/commit/166419cf912b3639abedfcb87ee8059920e5b221) Fixed `jsx: true` output not saying which JSX runtime to use, so a bundler compiling the JSX ignored `jsxImportSource` and the pragma options. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a `www.` URL linking when the character right before it is U+0085, which does not separate words. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks getting the wrong URL, or being dropped entirely, when a `[` earlier in the paragraph belongs to a code span, inline HTML, a pointed autolink, or a link that never resolves. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed text being dropped when an MDX expression contains the `]` that ends a reference label. — Thanks @Princesseuh!
- [7e28d6c](https://github.com/bruits/satteri/commit/7e28d6cd1251b92e337a6ab57b75aa55d923fad2) Fixed a `:directive` after an invalid bare URL being destroyed instead of parsed, as in `http://my_app.localhost:3000/admin`. — Thanks @Princesseuh!
- [c9ea0c9](https://github.com/bruits/satteri/commit/c9ea0c9e59d7e71afb6be97b378e787b0f3c96a8) Adds user-defined MDAST node types. A plugin can create a node with any `type` string, render it as an element through `data.hName` (or as text from a `value`), and reach every one of them from the new `custom` visitor key. Content nested inside a custom node stays visible to other plugins and to the HTML output. — Thanks @Princesseuh!
- [166419c](https://github.com/bruits/satteri/commit/166419cf912b3639abedfcb87ee8059920e5b221) Fixed `development: true` leaving out the line and column of elements that came from Markdown rather than from JSX written by hand. — Thanks @Princesseuh!
- [47768aa](https://github.com/bruits/satteri/commit/47768aaf8cb3566cbd0e231124bb0beff7212ded) Fixed whitespace between adjacent components disappearing in MDX compiled with static optimization enabled. — Thanks @Princesseuh!
- [46e2572](https://github.com/bruits/satteri/commit/46e25721656ec01fe494b62a3c2a5a48f1e45dfb) Fixed a `{` inside an MDX link destination or title raising a parse error when the tail holds an escaped or quoted `)`, as in `[a](\){)`, and stopped a link tail forming from a `[` that is backslash-escaped, inside a code span, already wrapped by another link, or in an earlier block. — Thanks @Princesseuh!
- [58add58](https://github.com/bruits/satteri/commit/58add589d8d9dc1c9a774e07519f0e3e7119df34) Fixed nodes created from raw string splices reporting garbage positions; they now report no position, like other plugin-created nodes. — Thanks @Princesseuh!
- [9bb585d](https://github.com/bruits/satteri/commit/9bb585d90298f647c4b85babe520e92b5b40c527) Fixed edits to a node another plugin removed being dropped silently instead of with the documented warning. — Thanks @Princesseuh!
- [2c14a38](https://github.com/bruits/satteri/commit/2c14a38e56d4903ccc2e933bb74c63d4c1426147) Fixed links and reference definitions whose parenthesized title holds an unescaped `(`, as in `[a](* (())`, not being parsed as links, and in MDX a `{` inside such a title no longer raises a parse error. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed two bare URLs separated by a `]` being merged into one over-long link, as in `[www.a.com]www.b.com`. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL swallowing the text after it when the two are separated by a non-breaking space or other Unicode whitespace. — Thanks @Princesseuh!
- [acee492](https://github.com/bruits/satteri/commit/acee492ddc0e703eaaed5169f52f7e7c7cf971ac) Fixed a link title being accepted with no whitespace after a `<...>` destination, so `[a](<u>"t")` is now plain text like in remark. — Thanks @Princesseuh!
- [6696c1c](https://github.com/bruits/satteri/commit/6696c1c28b3024c5c8df760cc5af51dd713663fc) Fixed `position` offsets being wrong in documents with multibyte characters. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed documents that use standalone carriage returns (`\r`) as line endings parsing differently from documents that use `\n`. Values such as inline code and definition titles now keep the document's own line endings instead of always reporting `\n`. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed the start offset of text in a table cell when it begins with an escaped pipe. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed an email address starting with `www.` linking as a URL instead of an email. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed the text after a GFM autolink being mangled when the URL ends on a character reference or a backslash, which could decode the wrong character, report an overlapping position, or swallow the inline HTML or emphasis that followed. — Thanks @Princesseuh!
- [c9f0757](https://github.com/bruits/satteri/commit/c9f07579e26a92f19d58afbc09336787f25e3587) Fixed MDX error messages reporting two different locations for documents that use lone carriage returns as line endings. — Thanks @Princesseuh!
- [50824f3](https://github.com/bruits/satteri/commit/50824f3dfbd8b67a2aaac0b643725fa9e3b624ba) Fixed every position being shifted by one in documents that start with a byte-order mark. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed fenced code block `lang` and `meta` splitting on whitespace that a character reference produced. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when the character just before it is an emoji or other astral punctuation. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.3.0, satteri-ast (Cargo)@0.5.0, satteri-mdxjs (Cargo)@0.3.9, satteri-plugin-api (Cargo)@0.5.0, satteri-pulldown-cmark (Cargo)@0.6.0

## 0.4.7 — 2026-07-08

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.4.2, satteri-mdxjs (Cargo)@0.3.8, satteri-plugin-api (Cargo)@0.4.2, satteri-pulldown-cmark (Cargo)@0.5.8

## 0.4.6 — 2026-06-29

### Patch changes

- [c6a9088](https://github.com/bruits/satteri/commit/c6a908875ae5161c86c592388a55f9caca9ed35b) Fixes plugin `ctx.source` being polluted with duplicated, concatenated content appended after the original document. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.2.2, satteri-ast (Cargo)@0.4.1, satteri-mdxjs (Cargo)@0.3.7, satteri-plugin-api (Cargo)@0.4.1, satteri-pulldown-cmark (Cargo)@0.5.7

## 0.4.5 — 2026-06-25

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.6, satteri-plugin-api (Cargo)@0.4.0, satteri-pulldown-cmark (Cargo)@0.5.6

## 0.4.4 — 2026-06-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.5, satteri-pulldown-cmark (Cargo)@0.5.5

## 0.4.3 — 2026-06-18

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.4.0, satteri-mdxjs (Cargo)@0.3.4, satteri-plugin-api (Cargo)@0.3.0, satteri-pulldown-cmark (Cargo)@0.5.4

## 0.4.2 — 2026-06-11

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.3.3, satteri-pulldown-cmark (Cargo)@0.5.3

## 0.4.1 — 2026-06-08

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.3.2, satteri-mdxjs (Cargo)@0.3.2, satteri-plugin-api (Cargo)@0.2.2, satteri-pulldown-cmark (Cargo)@0.5.2

## 0.4.0 — 2026-06-03

### Minor changes

- [5b45ec8](https://github.com/bruits/satteri/commit/5b45ec89862fd675070006ec7b8c3c64bee408ed) Disabled math parsing by default; pass `math: true` to re-enable inline `$...$` and display `$$...$$` math. — Thanks @Princesseuh!

### Patch changes

- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Made HAST plugins match MDAST when a transform targets a node removed or replaced earlier in the same pass: the stranded transform is now dropped with a warning instead of throwing a fatal error. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed `ctx.wrapNode()` dropping content: the wrapper's own children are now kept after the wrapped node, and `prependChild`/`appendChild` calls on a node in the same pass it is wrapped are applied instead of being silently dropped. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed a crash when a plugin returned a replacement node whose children included the node being visited (for example, wrapping a heading in a `<div>` that contains it). — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.3.1, satteri-mdxjs (Cargo)@0.3.1, satteri-plugin-api (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.5.1

## 0.3.0 — 2026-06-02

### Minor changes

- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.gfm` control. Footnotes can now be customized without requiring a plugin. `backContent` and `backLabel` each accept either a string template or a JS callback `(referenceNumber, rerunIndex) => string` for cases that need to branch on the index.
  
  ```ts
  // Disable footnotes, keep the rest of GFM.
  markdownToHtml(source, { features: { gfm: { footnotes: false } } });
  
  // String templates.
  markdownToHtml(source, {
    features: {
      gfm: {
        footnotes: {
          label: "Notes de bas de page",
          backContent: "↑",
          backLabel: "Retour à la référence {reference}",
        },
      },
    },
  });
  
  // Callbacks for per-backref control.
  markdownToHtml(source, {
    features: {
      gfm: {
        footnotes: {
          backLabel: (n, k) => (k > 1 ? `Retour ${n}-${k}` : `Retour ${n}`),
          backContent: (_n, k) => (k === 1 ? "↑" : `↑${k}`),
        },
      },
    },
  });
  ```
  
  In a string template, `{reference}` expands to the footnote number on the first backref and to `number-K` on repeated backrefs to the same definition. Template mode also appends `<sup>K</sup>` after the back content on reruns; callback mode skips the auto-sup and lets the callback return the final content. — Thanks @Princesseuh!
- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.math` control. `singleDollarTextMath: false` keeps single-`$` constructs as literal text (so prose can carry currency like "$50 to $100") while `$$ ... $$` still parses as display math.
  
  ```ts
  markdownToHtml(source, {
    features: { math: { singleDollarTextMath: false } },
  });
  ```
   — Thanks @Princesseuh!
- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too — in a single pass.
  
  A node returned from a visitor that passes existing children through (e.g. `{ ...node, children: [...node.children] }`) now keeps those children's identity, so a transform queued on a nested one in the same pass still applies. Previously this crashed with `patch targets node N inside a removed subtree`.
  
  Note: a visitor's own freshly-built nodes are not re-walked by that same visitor. Produce their final shape directly, or hand off to a later plugin (which sees the materialized tree). — Thanks @Princesseuh!
- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Adds an `mdx` cargo feature (enabled by default) across the Rust crates. Disabling it compiles out all MDX support. In the future, this will be used to ship a "lite" version of Sätteri for environments where MDX is not needed and bundle size is a concern.
  
  On Linux the native addon drops from ~2.99 MB to ~1.36 MB when disabling MDX. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.1, satteri-ast (Cargo)@0.3.0, satteri-mdxjs (Cargo)@0.3.0, satteri-plugin-api (Cargo)@0.2.0, satteri-pulldown-cmark (Cargo)@0.5.0

## 0.2.3 — 2026-05-19

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.3

## 0.2.2 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.2.0, satteri-ast (Cargo)@0.2.7, satteri-mdxjs (Cargo)@0.2.2, satteri-plugin-api (Cargo)@0.1.13, satteri-pulldown-cmark (Cargo)@0.4.1

## 0.2.1 — 2026-05-18

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.1, satteri-pulldown-cmark (Cargo)@0.4.0

## 0.2.0 — 2026-05-18

### Minor changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Added `elementAttributeNameCase` and `stylePropertyNameCase` options. Set `elementAttributeNameCase: "html"` to emit `class`/`for` instead of `className`/`htmlFor`, and `stylePropertyNameCase: "css"` to keep kebab-case keys in `style` objects. Defaults stay React-compatible. — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.2.0

## 0.1.15 — 2026-05-12

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.6, satteri-mdxjs (Cargo)@0.1.16, satteri-plugin-api (Cargo)@0.1.12, satteri-pulldown-cmark (Cargo)@0.3.6

## 0.1.14 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!
- Updated dependencies: satteri-arena (Cargo)@0.1.4, satteri-ast (Cargo)@0.2.5, satteri-mdxjs (Cargo)@0.1.15, satteri-plugin-api (Cargo)@0.1.11, satteri-pulldown-cmark (Cargo)@0.3.5

## 0.1.13 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.4, satteri-mdxjs (Cargo)@0.1.14, satteri-plugin-api (Cargo)@0.1.10, satteri-pulldown-cmark (Cargo)@0.3.4

## 0.1.12 — 2026-04-30

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.3, satteri-mdxjs (Cargo)@0.1.13, satteri-plugin-api (Cargo)@0.1.9, satteri-pulldown-cmark (Cargo)@0.3.3

## 0.1.11 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.2, satteri-mdxjs (Cargo)@0.1.12, satteri-plugin-api (Cargo)@0.1.8, satteri-pulldown-cmark (Cargo)@0.3.2

## 0.1.10 — 2026-04-29

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.2.1, satteri-mdxjs (Cargo)@0.1.11, satteri-plugin-api (Cargo)@0.1.7, satteri-pulldown-cmark (Cargo)@0.3.1

## 0.1.9 — 2026-04-29

### Patch changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Renamed `Options::ENABLE_CONTAINER_EXTENSIONS` to `Options::ENABLE_DIRECTIVE`. If you use this crate directly, update the option name; if you only consume satteri through the npm package or the high-level Rust API, no change is needed (the `features.directive` toggle keeps its name). — Thanks @Princesseuh!
- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.
  
  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.2.0, satteri-mdxjs (Cargo)@0.1.10, satteri-plugin-api (Cargo)@0.1.6, satteri-pulldown-cmark (Cargo)@0.3.0

## 0.1.8 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-arena (Cargo)@0.1.3, satteri-ast (Cargo)@0.1.5, satteri-mdxjs (Cargo)@0.1.9, satteri-plugin-api (Cargo)@0.1.5, satteri-pulldown-cmark (Cargo)@0.2.5

## 0.1.7 — 2026-04-27

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.4, satteri-mdxjs (Cargo)@0.1.8, satteri-plugin-api (Cargo)@0.1.4, satteri-pulldown-cmark (Cargo)@0.2.4

## 0.1.6 — 2026-04-17

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.7, satteri-pulldown-cmark (Cargo)@0.2.3

## 0.1.5 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.6, satteri-pulldown-cmark (Cargo)@0.2.2

## 0.1.4 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.3, satteri-mdxjs (Cargo)@0.1.5, satteri-plugin-api (Cargo)@0.1.3, satteri-pulldown-cmark (Cargo)@0.2.1

## 0.1.3 — 2026-04-16

### Patch changes

- Updated dependencies: satteri-mdxjs (Cargo)@0.1.4

## 0.1.2 — 2026-04-15

### Patch changes

- [bfb8968](https://github.com/bruits/satteri/commit/bfb89681df076d683a8c9cf6612b21195b06a566) Added `parseExpression()` to `mdxjsEsm` nodes, allowing ESM import/export statements to be parsed into ESTree ASTs. — Thanks @Princesseuh!
- Updated dependencies: satteri-mdxjs (Cargo)@0.1.3

## 0.1.1 — 2026-04-14

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2, satteri-mdxjs (Cargo)@0.1.2, satteri-plugin-api (Cargo)@0.1.2, satteri-pulldown-cmark (Cargo)@0.2.0

