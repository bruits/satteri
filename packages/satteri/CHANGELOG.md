# satteri

## 0.9.3 — 2026-06-25

### Patch changes

- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes a blank line inside a template literal or block comment in an MDX `import`/`export` causing an `Unterminated string` error. The blank line no longer ends the statement early. — Thanks @Princesseuh!
- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes inline math like `$\frac{-b}{2a}$` failing to compile in MDX. Braces inside `$...$` are now treated as math text, not a JSX expression. — Thanks @Princesseuh!
- [66e4f07](https://github.com/bruits/satteri/commit/66e4f0755eefabef2f8b9407d7a843a81b45ab49) Fixes Markdown plugins returning `rawHtml` with literal `{` or `}` rendering those braces as MDX escape fragments in `markdownToHtml`. — Thanks @snvtac for your first contribution 🎉!
- [fab4a2d](https://github.com/bruits/satteri/commit/fab4a2dbfe534d45fb7b3602d709418dcc2caf86) Fixes quotes inside a regex in an MDX JSX attribute (e.g. `ins={[/icon="[^"]+"/g]}`) causing a parse error. — Thanks @Princesseuh!
- [27c9023](https://github.com/bruits/satteri/commit/27c90239935f218103995a4d82a6473dc1d728f8) Fixes `headingAttributes` silently dropping parsed attributes. — Thanks @Princesseuh!

## 0.9.2 — 2026-06-23

### Patch changes

- [6128184](https://github.com/bruits/satteri/commit/61281847992173dcf37a588c5b1a49200ec28ace) Add prebuilt native bindings for more platforms: `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, and `win32-arm64-msvc`. — Thanks @Princesseuh!

## 0.9.1 — 2026-06-19

### Patch changes

- [64877f0](https://github.com/bruits/satteri/commit/64877f0dfa46fb0f752c8b3a9affc8c8552ade67) Adds a `data` option to `markdownToHtml`, `mdxToJs`, and `CompileOptions` that seeds the document data bag before plugins run. The same object is surfaced to plugins as `ctx.data` and returned as `result.data`, so values can be passed both into and out of a compile. — Thanks @Princesseuh!
- [855379c](https://github.com/bruits/satteri/commit/855379c7eb018e9c5acc69daa7a63f27dbb79e7f) Fix MDX `import`/`export` blocks being broken by a following whitespace-only line. A line containing only spaces or tabs now ends the ESM block exactly like an empty line, instead of being consumed as a statement continuation (which produced a `Could not parse esm with oxc` error). — Thanks @Princesseuh!
- [855379c](https://github.com/bruits/satteri/commit/855379c7eb018e9c5acc69daa7a63f27dbb79e7f) MDX parse errors now carry a source line and column. Previously, errors in `import`/`export` blocks dropped the position entirely, and errors in `{…}` expressions and JSX attributes were reported as a bare byte offset, so downstream tooling reported an unknown location. JSX attribute and spread expression errors now point at the offending attribute rather than the element's opening `<`. — Thanks @Princesseuh!
- [7c78426](https://github.com/bruits/satteri/commit/7c78426b0a21c8a3e41e6fed6605ceba60650826) Fixes a performance regression when not using any plugins. — Thanks @Princesseuh!

## 0.9.0 — 2026-06-18

### Minor changes

- [b2ae465](https://github.com/bruits/satteri/commit/b2ae465e41d87174455af65b2613c307233b8ac5) Improves performance when using plugins by using a new method of communication between Rust and JS. — Thanks @Princesseuh!

### Patch changes

- [6bcdf06](https://github.com/bruits/satteri/commit/6bcdf06a0ee267779180a2d89a27a31f2f4b5b81) `features.superscript` and `features.subscript` now render `^text^` as `<sup>text</sup>` and `~text~` as `<sub>text</sub>` as documented, instead of `<em>`. The MDAST now exposes dedicated `superscript` and `subscript` node types, which plugins can visit and construct. Plugins that previously matched these spans as `emphasis` nodes should switch to the new node types. — Thanks @morinokami for your first contribution 🎉!
- [d6e28f4](https://github.com/bruits/satteri/commit/d6e28f45623a37a74e694cb75e5a6e916c220677) Fixes a parse error when an MDX expression uses top-level `await`, such as `<Card data={await getData()} />`. — Thanks @Princesseuh!
- [9867bbc](https://github.com/bruits/satteri/commit/9867bbc9dc71f68c7c6aff5307fdd48f723ebdda) Add `ctx.parent(node)` and `ctx.indexOf(node)` to the MDAST and HAST plugin visitor contexts.

  `parent()` returns a node's parent (or `undefined` at the root) and is climbable to reach any ancestor;

  `indexOf()` returns a node's position within its parent's children. Together they make it possible to do operations depending on ancestry and siblings. — Thanks @Princesseuh!

- [0d36b24](https://github.com/bruits/satteri/commit/0d36b249d435940efaf95b03fa4fecd1a38a1c56) Aligns directive attribute type with `mdast-util-directive` by allowing nullish attribute values. — Thanks @HiDeoo!
- [efba0de](https://github.com/bruits/satteri/commit/efba0de3b74cba630071400fc769671ca150c183) Add the missing `position` and `data` properties to the `raw` hast node type. — Thanks @Princesseuh!
- [77b8b1d](https://github.com/bruits/satteri/commit/77b8b1d59dcaf712a607a956f3aadece32fec7e4) Add `ctx.data`, a document-scoped data bag shared across every plugin in the compile.

  Writes from one plugin are visible to later plugins, and the bag persists across the mdast→hast boundary, so hast plugins can read what mdast plugins wrote. After compilation the final state is returned on `result.data`. The bag lives entirely on the JS side, so any value is allowed (functions, class instances, `Map`/`Set`) and references are preserved, much like `vfile.data`. Specific keys can be typed by augmenting the `DataMap` interface via `declare module "satteri"`. — Thanks @Princesseuh!

## 0.8.2 — 2026-06-11

### Patch changes

- [42835bc](https://github.com/bruits/satteri/commit/42835bcad387064678421d5623067500c4cefa1c) Fixes a smart punctuation issue where double quotes could be rendered with the wrong direction when quoted text appeared next to text without whitespace. — Thanks @HiDeoo for your first contribution 🎉!

## 0.8.1 — 2026-06-08

### Patch changes

- [e58b500](https://github.com/bruits/satteri/commit/e58b500aecfce9c03e3a5045a2d5a063eb1f8203) Fixes a parsing error when a MDX attribute contained the closing tag of itself, e.g. `<Component attr="</Component>">`. The parser would incorrectly treat the `</Component>` as the closing tag of the component, instead of part of the attribute value. — Thanks @Princesseuh!
- [f41d32f](https://github.com/bruits/satteri/commit/f41d32f590e7763f7ba8199aead1e563503c8a9a) Fixes `ctx.setProperty(node, "children", [...])`, which used to throw an error. You can now set a node's children directly, and any other properties you set on the same node still take effect. — Thanks @Princesseuh!
- [67ac7b0](https://github.com/bruits/satteri/commit/67ac7b06aa270c22664cfa3c7a11d6bf37495529) Fixes `ctx.textContent()` not including inline math. A heading like `# Energy $E=mc^2$` would only return `Energy ` instead of `Energy E=mc^2`. — Thanks @Princesseuh!
- [67ac7b0](https://github.com/bruits/satteri/commit/67ac7b06aa270c22664cfa3c7a11d6bf37495529) Fixes several kinds of nodes getting mangled when a plugin would move or duplicate them. — Thanks @Princesseuh!
- [7979f1e](https://github.com/bruits/satteri/commit/7979f1ec93695a8b700272f75be967bdba29452b) Fixes a crash when a plugin replaces a node with a tree containing an empty text node in a document that has non-ASCII characters (e.g. `é`). — Thanks @HiDeoo for your first contribution 🎉!
- [f41d32f](https://github.com/bruits/satteri/commit/f41d32f590e7763f7ba8199aead1e563503c8a9a) Adds `ctx.insertChildAt(node, index, child)` and `ctx.removeChildAt(node, index)` for editing a node's children by position.

  `insertBefore`, `insertAfter`, `prependChild`, `appendChild`, and `insertChildAt` now also accept an array of nodes, so you can insert several at once. — Thanks @Princesseuh!

## 0.8.0 — 2026-06-03

### Minor changes

- [5b45ec8](https://github.com/bruits/satteri/commit/5b45ec89862fd675070006ec7b8c3c64bee408ed) Disabled math parsing by default; pass `math: true` to re-enable inline `$...$` and display `$$...$$` math. — Thanks @Princesseuh!

### Patch changes

- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Made HAST plugins match MDAST when a transform targets a node removed or replaced earlier in the same pass: the stranded transform is now dropped with a warning instead of throwing a fatal error. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed `ctx.wrapNode()` dropping content: the wrapper's own children are now kept after the wrapped node, and `prependChild`/`appendChild` calls on a node in the same pass it is wrapped are applied instead of being silently dropped. — Thanks @Princesseuh!
- [c91de73](https://github.com/bruits/satteri/commit/c91de73b75420934819c4488101aa9589be7f39c) Fixed a crash when a plugin returned a replacement node whose children included the node being visited (for example, wrapping a heading in a `<div>` that contains it). — Thanks @Princesseuh!

## 0.7.0 — 2026-06-02

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

- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) The `filename` option (and the `ctx.filename` it surfaced to plugins) is now `fileURL` and only accepts a `URL` instead of a string. Create one with `new URL('path/to/file', import.meta.url)`, convert a file path with `pathToFileURL('path/to/file')`, or pass an existing file URL directly.

  This change was made to avoid normalization issues across operating systems, enable the support of virtual paths and just generally promote a more consistent format over raw strings. — Thanks @Princesseuh!

- [8d84807](https://github.com/bruits/satteri/commit/8d84807fe572950f47f0017f68a3b753dd9e90c3) Adds granular `features.math` control. `singleDollarTextMath: false` keeps single-`$` constructs as literal text (so prose can carry currency like "$50 to $100") while `$$ ... $$` still parses as display math.

  ```ts
  markdownToHtml(source, {
    features: { math: { singleDollarTextMath: false } },
  });
  ```

  — Thanks @Princesseuh!

### Patch changes

- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Nested directives now transform correctly. When a plugin turns a directive into something else (for example a `containerDirective` visitor that renders both an outer `:::note` and a nested `:::tip` as asides), the inner one is transformed too — in a single pass.

  A node returned from a visitor that passes existing children through (e.g. `{ ...node, children: [...node.children] }`) now keeps those children's identity, so a transform queued on a nested one in the same pass still applies. Previously this crashed with `patch targets node N inside a removed subtree`.

  Note: a visitor's own freshly-built nodes are not re-walked by that same visitor. Produce their final shape directly, or hand off to a later plugin (which sees the materialized tree). — Thanks @Princesseuh!

- [c69e907](https://github.com/bruits/satteri/commit/c69e9073f3f101faf8058f05f6e6fea4466039fe) Fixes Markdown plugins that return raw Markdown or HTML (`{ raw }` / `{ rawHtml }`) sometimes inserting unnecessary nested `root` nodes into the MDAST tree. — Thanks @Princesseuh!
- [d6badad](https://github.com/bruits/satteri/commit/d6badad93105125904caeded0907f0c094b58fbd) Fixes `position` property always returning `undefined` on hast nodes. — Thanks @Princesseuh!
- [b8d8fa8](https://github.com/bruits/satteri/commit/b8d8fa8d56cfef1e1c35a5a37e9c61ed421d7bac) Directive labels now render full Markdown. `:::note[Custom **strong** Label]` shows bold text instead of literal `**` markers. Emphasis, links, inline code, and (in MDX) components and expressions all work inside a label now, on container, leaf, and text directives. Previously a label only understood inline code.

  Directives that end with an HTML block also close cleanly now. A `:::note` whose last line before the closing fence is `</details>` no longer leaks a stray `:::` into the output. — Thanks @Princesseuh!

- [18f269f](https://github.com/bruits/satteri/commit/18f269f216a8e46240f3e7d71ca52c99aee9a709) Fixed inline `style` custom properties (`--*`) being lowercased in MDX, which broke `var()` references to case-sensitive names like `--tmLabel` — Thanks @Princesseuh!

## 0.6.3 — 2026-05-21

### Patch changes

- [1c7b915](https://github.com/bruits/satteri/commit/1c7b915176669e12d9b93cb9d3ab0dc2b56f4b4a) Type `parseExpression()` as an actual ESTree `Program` instead of `Record<string, any>`. — Thanks @Princesseuh!

## 0.6.2 — 2026-05-20

### Patch changes

- [82928b3](https://github.com/bruits/satteri/commit/82928b32c79cf95141d4996a6a5ae82e1c02bccd) Export the MDX node types (`MdxJsxFlowElement`, `MdxJsxFlowElementHast`, and the rest) — Thanks @Princesseuh!

## 0.6.1 — 2026-05-19

### Patch changes

- [befcaf0](https://github.com/bruits/satteri/commit/befcaf044787316c7f86a98667719a41d79da849) Fix a crash when an MDX file defines a component with `export const`, `export function`, or `export class` and then uses it as a JSX tag. Previously the component would be treated as if it had to come from `props.components`, and rendering threw "Expected component X to be defined" unless you also passed it in. It now resolves to the locally-defined component as expected. — Thanks @Princesseuh!

## 0.6.0 — 2026-05-18

### Minor changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Added `elementAttributeNameCase` and `stylePropertyNameCase` options. Set `elementAttributeNameCase: "html"` to emit `class`/`for` instead of `className`/`htmlFor`, and `stylePropertyNameCase: "css"` to keep kebab-case keys in `style` objects. Defaults stay React-compatible. — Thanks @Princesseuh!

### Patch changes

- [f12e64e](https://github.com/bruits/satteri/commit/f12e64e12a5b6cc765252633c16b38f8c21e9282) Fixed MDX files that declare a layout via `export { default } from ...` or `export default` not rendering at runtime. — Thanks @Princesseuh!

## 0.5.1 — 2026-05-12

### Patch changes

- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed possible memory leak when a plugin threw during compilation. — Thanks @Princesseuh!
- [4a189f7](https://github.com/bruits/satteri/commit/4a189f77bdf55ab7b238810673ef88e6374d02a5) Fixed plugin-inserted MDX JSX elements compiling as literal HTML tags instead of routing through `_components`, which prevented user overrides via the `components` prop. — Thanks @Princesseuh!

## 0.5.0 — 2026-05-12

### Minor changes

- [adeb321](https://github.com/bruits/satteri/commit/adeb321c9a7c83c60cfa54fb5e886445d640721c) `markdownToHtml` and `mdxToJs` now return an object instead of a bare string. The first field carries the rendered output (`html`, or `code` for MDX), and a new `frontmatter` field exposes the first YAML or TOML frontmatter block in the document, or `null` if none.

  ```js
  // Before
  const html = markdownToHtml(source);

  // After
  const { html, frontmatter } = markdownToHtml(source);
  ```

  This makes it easier to then pass the frontmatter to a YAML / TOML library of your choice, without needing to extract it using a plugin. — Thanks @Princesseuh!

### Patch changes

- [26f2c22](https://github.com/bruits/satteri/commit/26f2c22945cf0998e69c88fc450c89a23f291c36) Add a fallback for WebContainer that downloads `@bruits/satteri-wasm32-wasi` on demand when none of the native or WASI bindings are reachable in the install. — Thanks @Princesseuh!

## 0.4.0 — 2026-05-07

### Minor changes

- [6f380d3](https://github.com/bruits/satteri/commit/6f380d346f9bc51d60213f84d51e3d8123f63a25) Added factory-shape support to `hastPlugins` and `mdastPlugins`: each entry can now be a function returning a plugin definition, called once per compile. This is useful for stateful plugins. — Thanks @Princesseuh!

## 0.3.5 — 2026-05-06

### Patch changes

- [22c4f06](https://github.com/bruits/satteri/commit/22c4f06e8923de01a371db798dbf39022737ad33) Fixes a rare case where plugins could produce corrupted output in very specific situations. — Thanks @Princesseuh!

## 0.3.4 — 2026-04-30

### Patch changes

- [80d21c8](https://github.com/bruits/satteri/commit/80d21c8b9bc7f7cb2f86c170d4fafac0d5d2a3b7) Fix a crash when an MDAST plugin returns a tree containing a directive
  (`containerDirective` / `leafDirective` / `textDirective`) and the surrounding
  document contains multi-byte text (e.g. Devanagari, CJK). — Thanks @Princesseuh!
- [80d21c8](https://github.com/bruits/satteri/commit/80d21c8b9bc7f7cb2f86c170d4fafac0d5d2a3b7) Reduced memory usage when using MDAST plugins. — Thanks @Princesseuh!

## 0.3.3 — 2026-04-30

### Patch changes

- [8e7642c](https://github.com/bruits/satteri/commit/8e7642cde7aa2c1b0e0b9a7676666f2c990ca7da) Fixed compilation crashing with `invalid type: map, expected a sequence` when an MDAST plugin returned a tree containing a directive node (`containerDirective`, `leafDirective`, `textDirective`). Directive children now round-trip through plugins correctly. — Thanks @Princesseuh!

## 0.3.2 — 2026-04-29

### Patch changes

- [bf7c5a0](https://github.com/bruits/satteri/commit/bf7c5a0bb9865f8147ea6b0815558df3ece0de08) Fixed SVG attributes names (e.g. `viewBox`, `fillOpacity`) being converted to lowercase when set on elements from JS plugins — Thanks @Princesseuh!
- [bf7c5a0](https://github.com/bruits/satteri/commit/bf7c5a0bb9865f8147ea6b0815558df3ece0de08) Fixed numeric property values (e.g. `width: 16`, `start: 5`) being silently dropped when set on elements from JS plugins. — Thanks @Princesseuh!

## 0.3.1 — 2026-04-29

### Patch changes

- [467bdf9](https://github.com/bruits/satteri/commit/467bdf9b523b1ff1f560499c4d4c769e9c888166) Fixed plugin-set `data` being lost or corrupted on MDAST and HAST nodes in certain cases. — Thanks @Princesseuh!

## 0.3.0 — 2026-04-29

### Minor changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) MDAST plugins can now set `data.hName`, `data.hProperties`, and `data.hChildren` on a node and have Sätteri render the corresponding HAST element, matching the rehype idiom.

  This is especially useful for rendering directives, given a `containerDirective`, an `hName` of `"aside"` and `hProperties` of `{ className: ["note"] }`, satteri will emit `<aside class="note">…</aside>`. — Thanks @Princesseuh!

### Patch changes

- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed a crash when an MDAST plugin called `ctx.setProperty(node, "data", …)` on certain specific node types (e.g. `paragraph`, `blockquote`, `delete`). The call now succeeds and the data round-trips through the conversion pipeline as expected. — Thanks @Princesseuh!
- [baae3b8](https://github.com/bruits/satteri/commit/baae3b83b56bf0fb4cd0b0d2f376627ff0267b8f) Fixed plugins silently dropping all but the last structural change against a given node. Multiple `insertBefore`/`insertAfter` calls on the same node, or sibling inserts paired with a `removeNode` on that same node, now all apply in the order they were issued.

  Combinations that don't have a sensible meaning, like modifying something inside a removed subtree, now report an error instead of silently dropping the change. — Thanks @Princesseuh!

## 0.2.8 — 2026-04-29

### Patch changes

- [1f92697](https://github.com/bruits/satteri/commit/1f9269712ad4276bdbf8c9d2f205d8029bea7c43) Added visitor support for `containerDirective`, `leafDirective`, and `textDirective` nodes. Plugin authors can now subscribe to directive nodes directly (with typed `name`/`attributes` and children).

  Removed the `root` visitor key. Plugins should subscribe to specific node types instead; a dedicated API for prepending or appending content at the document level will land separately. — Thanks @Princesseuh!

## 0.2.7 — 2026-04-27

### Patch changes

- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Various MDX parsing fixes:
  - Fixed non-ASCII content in MDX expressions/JSX inside containers (blockquotes, lists) being corrupted due to byte-by-byte char casting.
  - Fixed MDX-only paragraphs inside blockquotes not being unraveled (producing spurious `<p>` wrappers).
  - Fixed multiple JSX elements on one line only rendering the first element.
  - Multiple other cases of small inconsistencies with `@mdxjs/mdx`, notably in whitespace handling and node positions. — Thanks @Princesseuh!

- [f632abf](https://github.com/bruits/satteri/commit/f632abf4ac516f1c8bb3fc713f8894cab9be5d8f) Added granular smart punctuation options (`ENABLE_SMART_QUOTES`, `ENABLE_SMART_DASHES`, `ENABLE_SMART_ELLIPSES`) that can be enabled independently instead of the entire group. — Thanks @Princesseuh!
- [5736ca4](https://github.com/bruits/satteri/commit/5736ca45dd3eaf703e6d573f19274b42f1ca6cb9) Fixes many output inconsistencies with remark across Markdown, GFM, and MDX parsing, mostly found by extensive property-based fuzz testing. Notable areas: GFM bare-URL detection, MDX JSX flow vs inline classification, footnote numbering and section ordering, directive label inline parsing, list spread/tight handling, and reference link spans. — Thanks @Princesseuh!

## 0.2.6 — 2026-04-17

### Patch changes

- [11ffcfc](https://github.com/bruits/satteri/commit/11ffcfca6c8486a3744e37e0c19e78100925323e) Fixed unclosed `{` in a paragraph silently consuming later blocks as an MDX expression, and fixed literal `{` inside code spans being falsely reported as an unclosed MDX expression — Thanks @Princesseuh!

## 0.2.5 — 2026-04-16

### Patch changes

- [6f9f66f](https://github.com/bruits/satteri/commit/6f9f66fa75722c0b58f50783b5ac85fefd53a157) Fixed JSX inside MDX expression bodies, JSX inside `.map()` callbacks or other expressions is now compiled to `_jsx()` calls instead of being dropped or emitted as raw JSX — Thanks @Princesseuh!

## 0.2.4 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed hyphenated JSX element names (e.g. `<my-widget>`) written explicitly in MDX being incorrectly routed through the components provider and producing invalid JavaScript — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed script and style element contents being entity-escaped, which produced invalid output (e.g. `&lt;` inside `<script>`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed HAST property names not being mapped to their HTML attribute names during rendering (e.g. `className` now renders as `class`, `htmlFor` as `for`) — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed source positions being dropped for most node types during mdast-to-hast conversion, so hast plugins now see accurate positions across the tree — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed code blocks missing trailing newlines when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed footnote references and definitions not being rendered when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed table column alignment being dropped when using hast plugins — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed `code.value` in the MDAST tree including a trailing newline for well-formed fenced code blocks, which diverged from `remark-parse`. MDAST plugins inspecting `node.value` now see the same bytes as remark. — Thanks @Princesseuh!
- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed task list classes and checkbox inputs being missing when using hast plugins — Thanks @Princesseuh!

## 0.2.3 — 2026-04-16

### Patch changes

- [ae83450](https://github.com/bruits/satteri/commit/ae83450e535f965d45be64aa83bc12806acb827b) Fixed optimizeStatic silently collapsing elements that have runtime component overrides via `export const components` — Thanks @Princesseuh!

## 0.2.2 — 2026-04-15

### Patch changes

- [6f08f69](https://github.com/bruits/satteri/commit/6f08f69b3304ac12b643e6f582faa3c01859b400) Fixes missing `optionalDependencies` field. — Thanks @Princesseuh!

## 0.2.1 — 2026-04-15

### Patch changes

- [b0cdb9b](https://github.com/bruits/satteri/commit/b0cdb9b8a01eaff8fb4aa6d02cdeee080241bcfb) Added `parseExpression()` to `mdxjsEsm` nodes, allowing ESM import/export statements to be parsed into ESTree ASTs. — Thanks @Princesseuh!

## 0.2.0 — 2026-04-14

### Minor changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!

### Patch changes

- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Fixes browser export still bringing in Node code by accident. — Thanks @Princesseuh!
- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side. — Thanks @Princesseuh!
