# satteri

## 0.10.4 — 2026-08-19

### Patch changes

- [72d4710](https://github.com/bruits/satteri/commit/72d4710d34e54f0e4eb65cd217a1ac48f2d31e14) Fixed plugin-inserted elements being emitted as literal JSX tags, instead of going through `_components`, when another plugin-inserted node was marked as explicit JSX. — Thanks @Princesseuh!

## 0.10.3 — 2026-08-19

### Patch changes

- [1a052cf](https://github.com/bruits/satteri/commit/1a052cfc0a295d7adbbcd3c9a1173d8a1b34598e) Improved performance. — Thanks @Princesseuh!
- [2cf9aef](https://github.com/bruits/satteri/commit/2cf9aef81dacbc1121a0fbae913215eb7e36838f) Added `clobberPrefix` option to footnotes, mirroring `remark-rehype` — Thanks @Princesseuh!

## 0.10.2 — 2026-08-18

### Patch changes

- [c363b97](https://github.com/bruits/satteri/commit/c363b97ed83c548662eabf3d81011744a7915d6b) Fixed text being dropped when one paragraph held two footnote references whose identifier contains a backtick, or a `$` with the math feature enabled. The words between the two references, and the second reference itself, are no longer swallowed. — Thanks @Princesseuh!
- [c363b97](https://github.com/bruits/satteri/commit/c363b97ed83c548662eabf3d81011744a7915d6b) Fixed the footnote back-reference link landing inside an earlier paragraph when the definition ends in a list, code block, blockquote, table, or heading. It is now appended after that last block, as remark and GitHub do. — Thanks @Princesseuh!
- [003158b](https://github.com/bruits/satteri/commit/003158bba41196a16b2ccf013f3260af029a5fc8) Added `visitMdastHook`, `visitHastHook` and `normalizePlugins` to the exports, so a hand-driven plugin pipeline can run `before`/`after` hooks and resolve plugin factories the way `markdownToHtml` does. The diagnostic and hook types (`MdastDiagnostic`, `HastDiagnostic`, `MdastHookFn`, `HastHookFn`) are exported alongside them. — Thanks @Princesseuh!
- [c363b97](https://github.com/bruits/satteri/commit/c363b97ed83c548662eabf3d81011744a7915d6b) Fixed footnote links and IDs for identifiers containing non-ASCII characters or URL punctuation, which are now percent-encoded. `[^café]` links to `#user-content-fn-caf%C3%A9`, and an identifier holding a bare `%` no longer produces an invalid URL. — Thanks @Princesseuh!

## 0.10.1 — 2026-08-18

### Patch changes

- [1b09752](https://github.com/bruits/satteri/commit/1b09752d04b7e5494b583250330089a6401b377d) Added `{ fragment: true }` to `htmlToHast`, which parses the string as a fragment so the returned `root` holds its own top-level nodes instead of an implied `<html>`/`<head>`/`<body>`.
  
  Pass `space: "svg"` alongside it to read the fragment as foreign content, so `<circle />` self-closes and camel-cased tags like `clipPath` keep their casing instead of parsing as unknown HTML elements.
  
  ```ts
  import { htmlToHast } from "satteri";
  
  const tree = htmlToHast("<p>hi</p>", { fragment: true });
  // { type: "root", children: [{ type: "element", tagName: "p", ... }] }
  
  const icon = htmlToHast(`<circle cx="1" />`, { fragment: true, space: "svg" });
  ```
   — Thanks @Princesseuh!

## 0.10.0 — 2026-08-18

### Minor changes

- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Nodes handed to plugins are shared and now frozen: writing to a node's fields, `position`, `properties`/`attributes`, or `children` throws a `TypeError` instead of silently corrupting what later plugins see; go through the context methods to make changes.
  
  Keeping a node around after your visitor ran now works: it reads as the tree looked at that moment, instead of always throwing. The error only remains if you never read the node's content before the tree changed. Trees returned by `markdownToMdast`/`mdxToMdast`/`markdownToHast`/`mdxToHast` are your own data and stay fully mutable. — Thanks @Princesseuh!
- [e53e725](https://github.com/bruits/satteri/commit/e53e725e3eca758b5c65364b583c06a96d515510) Added a way to run a plugin only on some documents: a plugin factory now receives the file's `fileURL`, `sourceFormat`, `source` and `data`, and can return `null`, `undefined` or `false` to be left out for that document. Those skip values are also accepted anywhere a plugin entry can appear.
  
  ```js
  const onlyChangelogs = (ctx) =>
    ctx.fileURL?.pathname.endsWith("/CHANGELOG.md") ? rewriteVersions : null;
  
  markdownToHtml(source, { mdastPlugins: [onlyChangelogs, myPlugin] });
  ```
  
  Anything else in a plugin list now fails with an error naming the option and what it expected. — Thanks @Princesseuh!
- [2b85f56](https://github.com/bruits/satteri/commit/2b85f5602fc3340eef9faa3e41c66ff0a03ec8af) Adds `{ raw }` support to `wrapNode()` in HAST plugins: the HTML is parsed and the node is wrapped in the resulting element. — Thanks @Princesseuh!
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
- [eeb7f07](https://github.com/bruits/satteri/commit/eeb7f0778a7af229fd592dd027ddfe0723ba2b26) Source positions are now opt-in per plugin via `options: { position: true }`, and `node.position` is `undefined` in visitors otherwise. — Thanks @Princesseuh!
- [c9ea0c9](https://github.com/bruits/satteri/commit/c9ea0c9e59d7e71afb6be97b378e787b0f3c96a8) Adds user-defined MDAST node types. A plugin can create a node with any `type` string, render it as an element through `data.hName` (or as text from a `value`), and reach every one of them from the new `custom` visitor key. Content nested inside a custom node stays visible to other plugins and to the HTML output. — Thanks @Princesseuh!
- [8df3f76](https://github.com/bruits/satteri/commit/8df3f765b2df9cbfa1aa4130a126b9315e431c14) Added support for nested arrays in `mdastPlugins` and `hastPlugins`, so a package can export a bundle of plugins that you pass without spreading it. A bundle's plugins run in their own order, at the bundle's position. A factory can return a bundle as well as a single plugin, giving its plugins state they share with each other and reset per document.
  
  ```js
  import { typography } from "some-package"; // an array of plugins
  
  markdownToHtml(source, { mdastPlugins: [typography, myPlugin] });
  ```
   — Thanks @Princesseuh!
- [53fa9a9](https://github.com/bruits/satteri/commit/53fa9a9575f41eb858cf50b4298aea3a0c5f0f73) `ctx.replaceNode(node, newNode)` now accepts an array of nodes as well as a single node, matching `insertBefore`, `insertAfter`, `prependChild`, `appendChild` and `insertChildAt`. The nodes take the target's place in order, so `ctx.replaceNode(node, [a, b])` leaves `a` and `b` where `node` was. This works on both the MDAST and HAST visitor contexts. — Thanks @Princesseuh!
- [2ac113e](https://github.com/bruits/satteri/commit/2ac113e9851dfd15340a999f9a1e829a9d2b0f8f) Added `position: false` to `markdownToMdast`, `mdxToMdast`, `markdownToHast`, and `mdxToHast`, which skips recording `node.position`. On a 1 MB document that halves both the time to build a tree and the memory it occupies, so it is worth passing whenever nothing downstream reads positions.
  
  ```ts
  const tree = markdownToMdast(source, { position: false });
  ```
   — Thanks @Princesseuh!
- [63fbb77](https://github.com/bruits/satteri/commit/63fbb77a16b88d4df4928ed07e943752e87fff17) Plugins now splice strings with a single shape, `{ raw: string, mdxExpressions?: boolean }`, accepted by visitor return values and every structural mutator (`replace`, `insertBefore`, `insertAfter`, `prependChild`, `appendChild`, `wrapNode`). The string is re-parsed in place of the node.
  
  `mdxExpressions` (default `true`) controls what `{…}` means when the document is MDX: live expressions by default, or literal text with `mdxExpressions: false` — the right choice when injecting generated HTML whose braces are not expressions, like a Mermaid decision node `C{JWT valid?}` or math renderer output. Plain Markdown has no expressions, so the option is a no-op there.
  
  `{ rawHtml: string }` is deprecated; it keeps working and behaves exactly like `{ raw, mdxExpressions: false }`.
  
  ```ts
  defineMdastPlugin({
    code(node) {
      if (node.lang !== "mermaid") return;
      return { raw: renderMermaid(node.value), mdxExpressions: false };
    },
  });
  ```
   — Thanks @Princesseuh!
- [d8639d6](https://github.com/bruits/satteri/commit/d8639d64efa50f2adf2f88f6a4928559d2a30836) Added `htmlToHast`, which parses an HTML string into a HAST tree (elements, text, comments, doctype) with the same spec-compliant parsing a browser does. The result is a `root` wrapping the implied `<html>` subtree.
  
  ```ts
  import { htmlToHast } from "satteri";
  
  const tree = htmlToHast("<p>hi</p>");
  // { type: "root", children: [{ type: "element", tagName: "html", ... }] }
  ```
   — Thanks @IEvangelist for your first contribution 🎉!
- [6050fc4](https://github.com/bruits/satteri/commit/6050fc40a3b546f08817277c9adb816ec9bfe938) Adds `before`/`after` lifecycle hooks to plugins that run exactly once per document. — Thanks @Princesseuh!

### Patch changes

- [ac99c4f](https://github.com/bruits/satteri/commit/ac99c4f9ecf4e2fa3b5eb1dbf069160f1ba7a6f1) Improved HTML rendering performance with faster character escaping. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a line holding only a vertical tab or form feed counting as a blank line, which split paragraphs and let a definition run past its destination. — Thanks @Princesseuh!
- [ac99c4f](https://github.com/bruits/satteri/commit/ac99c4f9ecf4e2fa3b5eb1dbf069160f1ba7a6f1) Improved Markdown to HTML performance when no HAST plugins run and nothing sets `hName`, `hProperties`, or `hChildren` (which heading attributes do). — Thanks @Princesseuh!
- [88fbb7e](https://github.com/bruits/satteri/commit/88fbb7e45482f9ba53d4478e6565c3e75b0350fd) Fixes a crash in the browser and bundler builds where loading Sätteri could fail with `WebAssembly.Compile is disallowed on the main thread, if the buffer size is larger than 4KB`. The WebAssembly module now initializes asynchronously instead of compiling synchronously on the main thread. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks sometimes missing position information, such as a bare URL after an unclosed `[`. — Thanks @Princesseuh!
- [c9985d9](https://github.com/bruits/satteri/commit/c9985d93b5ee23aff07491360be83d4a3412f18b) Fixed `development: true` line and column numbers, missing-component references, and MDX parse error locations being wrong in documents with multibyte or emoji characters. — Thanks @Princesseuh!
- [f868e26](https://github.com/bruits/satteri/commit/f868e26e8c07a5e30b90b16b554835f73f37d0c0) Fixed React-cased SVG property names like `strokeLinecap` and `strokeLinejoin` leaking into HTML output as-is instead of serializing as `stroke-linecap` / `stroke-linejoin`. — Thanks @gtritchie!
- [7441ecd](https://github.com/bruits/satteri/commit/7441ecd029d800c567d5c5c9d102bd0bfc0a9e9e) Fixed a defined footnote reference like `[^x](y)` parsing as a link instead of a footnote reference followed by text. — Thanks @Princesseuh!
- [7e9ac4c](https://github.com/bruits/satteri/commit/7e9ac4c38b7cd4ede2eaf4353765d74e905e45ba) Fixed very deeply nested documents crashing the process instead of compiling. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX expressions failing to parse when a string inside them is continued over a CRLF line ending with a backslash. — Thanks @Princesseuh!
- [ac99c4f](https://github.com/bruits/satteri/commit/ac99c4f9ecf4e2fa3b5eb1dbf069160f1ba7a6f1) Improved parsing performance for documents with few or no autolink candidates. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when a character reference supplies its first character, as in `&#104;ttp://example.com`. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed MDX line comments and `import`/`export` blocks swallowing the lines after them in files that use lone carriage returns as line endings. — Thanks @Princesseuh!
- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Editing a node that belongs to a different document (a node kept from a previous compile, or an mdast node used in a hast plugin) now fails the compile with `invalid node id`. A few pathological edits now throw `unsupported patch shape`, most notably replacing a node with new content that reuses that same node while another plugin edits something inside it in the same pass, and inserting a sibling next to the root.
  
  Edits to nodes that another plugin removed in the same pass are still just dropped with a warning, and replacing, removing, or wrapping the root keeps working. — Thanks @Princesseuh!
- [d6dbbad](https://github.com/bruits/satteri/commit/d6dbbad1d47e43f10391b3e00792078da49bdfc7) Fixed an email overlapping a `www.` link swallowing the link, like `user@www.example.org` after an unclosed bracket. — Thanks @Princesseuh!
- [be2c1a1](https://github.com/bruits/satteri/commit/be2c1a168fdcc548b0c39980a3e4be1634acae8d) Fixed emphasis being parsed around a `~` when GFM is disabled, so `a*~*` now stays plain text. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed GFM autolinks losing their positions when smart punctuation is enabled. — Thanks @Princesseuh!
- [18d123b](https://github.com/bruits/satteri/commit/18d123bb749d4f6fb0fca4fc1e79129761958873) Fixed a `{` inside an MDX link destination or title raising a parse error when the link tail spans more than one line, as in `[a](/u\n"ti{tle")`. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed at the end of a line being dropped from the text. — Thanks @Princesseuh!
- [39bc97f](https://github.com/bruits/satteri/commit/39bc97fdd2ae4d65baf4f42930b383c9a1cb7185) Fixed a Promise in a plugin list being silently ignored instead of failing with a clear error, and gave malformed hast element visitors a real error message. — Thanks @Princesseuh!
- [5c4cd17](https://github.com/bruits/satteri/commit/5c4cd170b2e4d0db4fb9f610fc15802aa2757fd9) Fixed `elementAttributeNameCase: "html"` leaving a nested `<svg>` element's own React-cased attributes (like `strokeWidth`) unconverted on the MDX compile path; the SVG schema now covers the `<svg>` element itself, not just its descendants. — Thanks @gtritchie!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed inline code ending at the wrong backtick when its content looks like a URL. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed emphasis and character references being lost after a GFM autolink whose URL ends in a backslash. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved `markdownToMdast` and `markdownToHast` performance by reading each node from the wire buffer once while building the tree. — Thanks @Princesseuh!
- [eeb7f07](https://github.com/bruits/satteri/commit/eeb7f0778a7af229fd592dd027ddfe0723ba2b26) Faster parsing, MDX compilation, and plugin execution. — Thanks @Princesseuh!
- [f0c24d7](https://github.com/bruits/satteri/commit/f0c24d768f151fd1d171a1ebc1dd5170820588f8) Improved HAST property types in plugins: `node.properties.href`, `className`, `start` and every other known property are now typed individually, so reading one no longer needs a `typeof` guard to narrow it.
  
  ```ts
  element: {
    filter: ["a"],
    visit(node, ctx) {
      if (node.properties.href?.startsWith("http")) {
        // ...
      }
    },
  }
  ```
   — Thanks @Princesseuh!
- [2ac113e](https://github.com/bruits/satteri/commit/2ac113e9851dfd15340a999f9a1e829a9d2b0f8f) Walking a tree from `markdownToMdast`, `mdxToMdast`, `markdownToHast`, `mdxToHast`, or `htmlToHast` is roughly 4x faster. — Thanks @Princesseuh!
- [1126ad0](https://github.com/bruits/satteri/commit/1126ad0dc303de1f3f3eeccfb8355bd0b99d2eb9) Fixed a tight definition list gluing a definition's continuation paragraph onto the first block with no separator. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed a vertical tab or form feed standing in for a space in an ATX heading opener, a task list marker, an HTML tag, or a link or footnote label. — Thanks @Princesseuh!
- [6a1eaec](https://github.com/bruits/satteri/commit/6a1eaecb25e442d26bc6ee90ac63bdd28c4bd465) Fixed `wrapNode()` not accepting the `{ raw }` shape that every other structural mutator takes. — Thanks @Princesseuh!
- [204fb3a](https://github.com/bruits/satteri/commit/204fb3aac413201e6a99bc0bfc54c4e8d199d425) Fixed documents with many unclosed parenthesized link titles taking quadratic time to parse. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed a zero-width no-break space being dropped when it starts a text value. — Thanks @Princesseuh!
- [166419c](https://github.com/bruits/satteri/commit/166419cf912b3639abedfcb87ee8059920e5b221) Fixed `jsx: true` output not saying which JSX runtime to use, so a bundler compiling the JSX ignored `jsxImportSource` and the pragma options. — Thanks @Princesseuh!
- [c16f684](https://github.com/bruits/satteri/commit/c16f684995079b4dc9d62c29ef7a599f2f7b4303) Fixes an issue with MDAST plugin visitors receiving `undefined` instead of an empty array for the `children` property of empty parent nodes. — Thanks @HiDeoo!
- [35913b9](https://github.com/bruits/satteri/commit/35913b9694a3d5cf461382e55ac4470ee52be22c) Fixed compiles that produced very large plugin output permanently retaining tens of megabytes of buffer memory. — Thanks @Princesseuh!
- [2e3ed23](https://github.com/bruits/satteri/commit/2e3ed23aa0e2489c4ce667cb39eb29259664692d) Faster Markdown-to-HTML rendering, most noticeably on prose-heavy documents where GFM autolink scanning dominated: a 200KB CommonMark document renders about 7% faster end to end. — Thanks @Princesseuh!
- [3068358](https://github.com/bruits/satteri/commit/30683586e758d13c77c3c7dfc0f5ca421600852b) Fixed compile results being typed as synchronous when options come through a variable typed as the general options interface. — Thanks @Princesseuh!
- [64f3d5f](https://github.com/bruits/satteri/commit/64f3d5f8666851494195ebd150bfa47df4da56e9) Fixes inline code being mangled when it contains directive-like syntax. With directives enabled, writing something like `` `:foo[` `` followed by more inline code no longer merges the two code spans or drops a backtick: a `:` inside a code span is now treated as literal text, so you can safely show directive syntax in code. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a `www.` URL linking when the character right before it is U+0085, which does not separate words. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed GFM autolinks getting the wrong URL, or being dropped entirely, when a `[` earlier in the paragraph belongs to a code span, inline HTML, a pointed autolink, or a link that never resolves. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed text being dropped when an MDX expression contains the `]` that ends a reference label. — Thanks @Princesseuh!
- [7e28d6c](https://github.com/bruits/satteri/commit/7e28d6cd1251b92e337a6ab57b75aa55d923fad2) Fixed a `:directive` after an invalid bare URL being destroyed instead of parsed, as in `http://my_app.localhost:3000/admin`. — Thanks @Princesseuh!
- [166419c](https://github.com/bruits/satteri/commit/166419cf912b3639abedfcb87ee8059920e5b221) Fixed `development: true` leaving out the line and column of elements that came from Markdown rather than from JSX written by hand. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved parsing performance for documents with many link reference definitions inside lists or blockquotes. — Thanks @Princesseuh!
- [47768aa](https://github.com/bruits/satteri/commit/47768aaf8cb3566cbd0e231124bb0beff7212ded) Fixed whitespace between adjacent components disappearing in MDX compiled with static optimization enabled. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed a hard line break inside an image label adding a stray newline to the image's alt text. — Thanks @Princesseuh!
- [8df3f76](https://github.com/bruits/satteri/commit/8df3f765b2df9cbfa1aa4130a126b9315e431c14) Fixed `markdownToHtml`, `mdxToJs` and `markdownToJs` being typed as synchronous when a plugin list mixed sync and async plugins. The result is now correctly typed as a `Promise`. — Thanks @Princesseuh!
- [9a164f1](https://github.com/bruits/satteri/commit/9a164f110f2d01c525f9f5c03376508bd227e860) Fixes footnotes being ignored inside directives. A footnote reference nested in a rendered directive (e.g. `:::note … [^id] … :::`) now works like anywhere else (it renders as a footnote link and its definition appears in the footnotes section) instead of being left as literal `[^id]` text. — Thanks @Princesseuh!
- [2b85f56](https://github.com/bruits/satteri/commit/2b85f5602fc3340eef9faa3e41c66ff0a03ec8af) Fixed `wrapNode()` silently misplacing or dropping the node when given a parent that cannot hold children: an `html` node, a leaf-shaped custom node, or a void element like `<img>`. These now throw an error explaining what to use instead, and `parentNode` only accepts parent-capable nodes in TypeScript, so most of them are caught before running.
  
  Wrapping in a container is unchanged: `blockquote` and friends, a HAST element, an MDX JSX element, a custom node declaring a `children` array, and the root itself all keep working. — Thanks @Princesseuh!
- [46e2572](https://github.com/bruits/satteri/commit/46e25721656ec01fe494b62a3c2a5a48f1e45dfb) Fixed a `{` inside an MDX link destination or title raising a parse error when the tail holds an escaped or quoted `)`, as in `[a](\){)`, and stopped a link tail forming from a `[` that is backslash-escaped, inside a code span, already wrapped by another link, or in an earlier block. — Thanks @Princesseuh!
- [58add58](https://github.com/bruits/satteri/commit/58add589d8d9dc1c9a774e07519f0e3e7119df34) Fixed nodes created from raw string splices reporting garbage positions; they now report no position, like other plugin-created nodes. — Thanks @Princesseuh!
- [9bb585d](https://github.com/bruits/satteri/commit/9bb585d90298f647c4b85babe520e92b5b40c527) Fixed edits to a node another plugin removed being dropped silently instead of with the documented warning. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved parsing performance for documents with paragraphs inside lists, blockquotes, and other containers. — Thanks @Princesseuh!
- [2c14a38](https://github.com/bruits/satteri/commit/2c14a38e56d4903ccc2e933bb74c63d4c1426147) Fixed links and reference definitions whose parenthesized title holds an unescaped `(`, as in `[a](* (())`, not being parsed as links, and in MDX a `{` inside such a title no longer raises a parse error. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed two bare URLs separated by a `]` being merged into one over-long link, as in `[www.a.com]www.b.com`. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL swallowing the text after it when the two are separated by a non-breaking space or other Unicode whitespace. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved MDX to JavaScript compilation performance. — Thanks @Princesseuh!
- [acee492](https://github.com/bruits/satteri/commit/acee492ddc0e703eaaed5169f52f7e7c7cf971ac) Fixed a link title being accepted with no whitespace after a `<...>` destination, so `[a](<u>"t")` is now plain text like in remark. — Thanks @Princesseuh!
- [6696c1c](https://github.com/bruits/satteri/commit/6696c1c28b3024c5c8df760cc5af51dd713663fc) Fixed `position` offsets being wrong in documents with multibyte characters. — Thanks @Princesseuh!
- [291369a](https://github.com/bruits/satteri/commit/291369a71fddf3fc6be272ca799d51422fcb88e3) Fixed edits to a node kept from a previous compile silently changing an unrelated node instead of failing with `invalid node id`. — Thanks @Princesseuh!
- [abe1ee9](https://github.com/bruits/satteri/commit/abe1ee90dfe25dca52d98169c170d9ed138e28ea) Fixed documents that use standalone carriage returns (`\r`) as line endings parsing differently from documents that use `\n`. Values such as inline code and definition titles now keep the document's own line endings instead of always reporting `\n`. — Thanks @Princesseuh!
- [fdaa021](https://github.com/bruits/satteri/commit/fdaa0219966afee3b5d49e95feaa96318b857cf5) Fixed a node losing its position when an async HAST visitor returns a new text value. — Thanks @Princesseuh!
- [5a581ad](https://github.com/bruits/satteri/commit/5a581ad8eae90a7eef102d7727b7fe9f6a7d1893) Fixed the start offset of text in a table cell when it begins with an escaped pipe. — Thanks @Princesseuh!
- [a27c06d](https://github.com/bruits/satteri/commit/a27c06db317606172a4dab5675de0b265793acb8) Fixed an email address starting with `www.` linking as a URL instead of an email. — Thanks @Princesseuh!
- [9094edd](https://github.com/bruits/satteri/commit/9094edd70cbf49f28444838afc7c489ddf068c09) Improved `markdownToMdast` and `markdownToHast` performance on documents containing non-ASCII characters, which previously fell off a fast path and cost more than twice as much to decode. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed the text after a GFM autolink being mangled when the URL ends on a character reference or a backslash, which could decode the wrong character, report an overlapping position, or swallow the inline HTML or emphasis that followed. — Thanks @Princesseuh!
- [137ff48](https://github.com/bruits/satteri/commit/137ff48da7d4a7422cadb3c82b9b7e987aa87e23) Faster across the board: parsing is ~10% cheaper, editing the tree from plugins now costs proportionally to how much you change rather than how big the document is (3 edits on a 115KB document: ~160µs → under 50µs), reading nodes inside plugins is 40-75% faster, and memory stays flat under sustained workloads. — Thanks @Princesseuh!
- [c9f0757](https://github.com/bruits/satteri/commit/c9f07579e26a92f19d58afbc09336787f25e3587) Fixed MDX error messages reporting two different locations for documents that use lone carriage returns as line endings. — Thanks @Princesseuh!
- [50824f3](https://github.com/bruits/satteri/commit/50824f3dfbd8b67a2aaac0b643725fa9e3b624ba) Fixed every position being shifted by one in documents that start with a byte-order mark. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed fenced code block `lang` and `meta` splitting on whitespace that a character reference produced. — Thanks @Princesseuh!
- [419e711](https://github.com/bruits/satteri/commit/419e711fd4e3092c84fff462d3bbbae406a09472) With smart punctuation enabled, an unmatched close-flanking double quote (like the inch mark in `24" monitor`) now renders as a closing curly quote instead of an opening one. A double quote after a digit no longer opens a quotation, so dimension notation like `24"x36"` closes throughout. — Thanks @Princesseuh!
- [ac2b172](https://github.com/bruits/satteri/commit/ac2b17274772147a15863439c0484861f656cc13) Fixed a bare URL or email not linking when the character just before it is an emoji or other astral punctuation. — Thanks @Princesseuh!

## 0.9.5 — 2026-07-08

### Patch changes

- [ef3974e](https://github.com/bruits/satteri/commit/ef3974e86b080c1e88f0e92122ffd28f08e0ad37) Exposes a `ctx.sourceFormat` property on both mdast and hast plugin contexts. It is `"markdown"` when the plugin runs during a Markdown compile (`markdownToHtml`) and `"mdx"` during an MDX compile (`mdxToJs`), letting a plugin shared between both pipelines branch on which format it is handling. — Thanks @Princesseuh!
- [92c006a](https://github.com/bruits/satteri/commit/92c006afd60904122bbc55f524f36901fa05c517) Fixed `satteri` failing to bundle due to wasm dependencies — Thanks @The-LukeZ for your first contribution 🎉!
- [a579002](https://github.com/bruits/satteri/commit/a57900229b3b1c9b0675b4fb1bcc05ee7e801387) Set the license field in the package.json — Thanks @ghostdevv!

## 0.9.4 — 2026-06-29

### Patch changes

- [c6a9088](https://github.com/bruits/satteri/commit/c6a908875ae5161c86c592388a55f9caca9ed35b) Fixes plugin `ctx.source` being polluted with duplicated, concatenated content appended after the original document. — Thanks @Princesseuh!
- [65e0758](https://github.com/bruits/satteri/commit/65e0758e293b2c3bcfe3767770fad3daaf5fdb69) Exposes the `MdastVisitorContext` type from the `satteri` package. — Thanks @HiDeoo!
- [07ee532](https://github.com/bruits/satteri/commit/07ee53293af76d0dcddbac961ad35337c5500e74) Fixes JSX nested in an MDX attribute expression (e.g. `prop={<p>hi</p>}` or `title={<>x</>}`) being emitted as raw, un-lowered JSX, which produced invalid JavaScript. Also fixes quotes and apostrophes in such JSX text (e.g. `prop={<p>Acme Corp.'s "best" tool</p>}`) being mis-scanned as JS string literals and causing a parse error — the expression scanner now consumes a JSX element's children as text. — Thanks @vaneenige for your first contribution 🎉!
- [2be5f6b](https://github.com/bruits/satteri/commit/2be5f6bdd43ee2d66381b12920cf3ee2c45a3905) Updated `binding.browser.ts` to export functions from `browser.js` — Thanks @noClaps for your first contribution 🎉!

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
