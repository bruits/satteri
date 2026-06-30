---
title: "Quick start"
description: "Compile your first Markdown document."
section: "getting-started"
order: 20
---

## Install

{{ install pkg="satteri" /}}

See [Installation](/docs/installation/) for runtime support and browser notes.

## Compile a document

Pass a Markdown string to `markdownToHtml`. You get back the rendered HTML and any frontmatter the document had.

```js
import { markdownToHtml } from "satteri";

const { html } = markdownToHtml("# Hello, *world*");
console.log(html);
// <h1>Hello, <em>world</em></h1>
```

That's the whole API for the basic case. Options go in a second argument:

```js
import { markdownToHtml } from "satteri";

const { html, frontmatter } = markdownToHtml(source, {
  features: {
    gfm: true,
    frontmatter: true,
    math: true,
  },
});
```

## Using plugins

A plugin is an object with a `name` and a visitor for each node type you want to act on. The visitor gets a read-only view of the node and a `ctx` object that records mutations.

```js
import { markdownToHtml, defineMdastPlugin } from "satteri";

const stripInlineCode = defineMdastPlugin({
  name: "strip-inline-code",
  inlineCode(node, ctx) {
    ctx.replaceNode(node, { type: "text", value: node.value });
  },
});

const { html } = markdownToHtml("Use `let` instead of `var`.", {
  mdastPlugins: [stripInlineCode],
});
// <p>Use let instead of var.</p>
```

For more, see the [Plugins](/docs/plugins/) guide, the [Entry points](/docs/entry-points/) reference, and the full list of [Options](/docs/options/).
