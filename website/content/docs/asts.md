---
title: "Syntax trees"
description: "The two trees Sätteri exposes to plugins."
section: "concepts"
order: 10
---

Sätteri parses Markdown into an **MDAST** (Markdown Abstract Syntax Tree), then converts it to a **HAST** (Hypertext Abstract Syntax Tree) before serializing to HTML. Plugins can hook into either stage.

## MDAST

MDAST nodes describe Markdown semantics: a `heading` has a `depth`, a `list` is `ordered` or not, an `image` has an `alt` and `url`, etc. Sätteri uses the same node shapes as [mdast-util-from-markdown](https://github.com/syntax-tree/mdast-util-from-markdown) so existing remark code is familiar.

```js
{
  type: "heading",
  depth: 2,
  children: [{ type: "text", value: "Hello" }],
}
```

Operate on MDAST for Markdown-level work: collecting headings into a table of contents, replacing a custom shortcode, validating frontmatter.

## HAST

HAST nodes describe HTML semantics: an `element` has a `tagName` and `properties`, a `text` has a `value`. The shapes match [mdast-util-to-hast](https://github.com/syntax-tree/mdast-util-to-hast).

```js
{
  type: "element",
  tagName: "h2",
  properties: { id: "hello" },
  children: [{ type: "text", value: "Hello" }],
}
```

Operate on HAST for HTML-level work: adding attributes, wrapping elements, rewriting URLs.

## Why both

A shortcode that expands into a link belongs at the MDAST stage. Adding `target="_blank"` to external `<a>` tags belongs at HAST. Splitting the pipeline keeps each plugin at the level of abstraction it actually needs.

Pass MDAST plugins under `mdastPlugins` and HAST plugins under `hastPlugins`. Sätteri runs the MDAST stage first, then converts to HAST, then runs the HAST stage.
