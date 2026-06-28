---
title: "Entry points"
description: "The functions that parse, compile, and evaluate Markdown and MDX, plus their result shape."
section: "reference"
order: 3
---

Sätteri's entry points parse a source string and return a result object. They run synchronously unless a plugin has an async visitor, in which case they return a `Promise` of the same result (see [Async plugins](/docs/plugin-api/#async-plugins)). Each one accepts an options argument — see [Options](/docs/options/) for the full reference.

## markdownToHtml

```ts
markdownToHtml(source: string, options?: CompileOptions): MarkdownToHtmlResult;
```

Parse Markdown and render HTML.

```js
import { markdownToHtml } from "satteri";

const { html, frontmatter, data } = markdownToHtml("# Hello, *world*");
// html === "<h1>Hello, <em>world</em></h1>"
```

## mdxToJs

MDX is a programming language: `mdxToJs` compiles it to JavaScript and `evaluate` runs that JavaScript. Treat MDX like code you execute — never compile or evaluate MDX from authors you don't trust.

```ts
mdxToJs(source: string, options?: MdxCompileOptions): MdxToJsResult;
```

Parse MDX and compile it to JavaScript module source. The compiled code is on `code` (not `html`).

```js
import { mdxToJs } from "satteri";

const { code } = mdxToJs("# Hello\n\n<MyComponent />");
```

## Result shape

Both functions return an object, never a bare string:

```ts
interface MarkdownToHtmlResult {
  html: string; // rendered HTML
  frontmatter: Frontmatter | null;
  data: Data; // the document data bag
}

interface MdxToJsResult {
  code: string; // compiled JS module source
  frontmatter: Frontmatter | null;
  data: Data;
}
```

`frontmatter` is the parsed block at the top of the document, or `null` if there is none — see [Frontmatter](/docs/features/#frontmatter) for its shape. `data` is the [document data bag](/docs/options/#data).

## evaluate

Compile and run MDX in one step, returning the module's exports (including `default`, the component). Pass a JSX runtime:

```js
import { evaluate } from "satteri";
import * as runtime from "react/jsx-runtime";

const { default: Content } = evaluate("# Hello\n\n<Sparkle />", { ...runtime });
```

## Trees without compiling

To get a plain JavaScript AST without running plugins or rendering, use the tree functions. Each parses the source and returns a materialized tree directly (not a result object), and accepts only a `features` option.

```ts
markdownToMdast(source: string, options?: { features?: Features }): MdastNode;
mdxToMdast(source: string, options?: { features?: Features }): MdastNode;
markdownToHast(source: string, options?: { features?: Features }): HastNode;
mdxToHast(source: string, options?: { features?: Features }): HastNode;
```

```js
import { markdownToMdast } from "satteri";

const tree = markdownToMdast("# Hello");
tree.children[0].type; // "heading"
tree.children[0].depth; // 1
```

This is useful when you want Sätteri's fast native parsing but another pipeline (e.g. remark plugins and `remark-stringify`) for the rest. The returned tree is plain objects, yours to keep — see [Node lifetime](/docs/plugin-api/#node-lifetime) for why that matters.
