---
title: "Plugins"
description: "Transform Markdown by hooking into the MDAST or HAST stage."
section: "guides"
order: 10
---

A plugin is an object with a `name` and one or more visitors. You wrap it with `defineMdastPlugin` or `defineHastPlugin` for type inference, then pass it to `markdownToHtml`.

See [Syntax trees](/docs/asts/) for what MDAST and HAST are and when to use each.

## MDAST plugins

An MDAST visitor is a function keyed by node type. It receives the node plus a `ctx` object that records mutations.

```js
import { markdownToHtml, defineMdastPlugin } from "satteri";

const emojis = defineMdastPlugin({
  name: "emojis",
  text(node, ctx) {
    if (node.value.includes(":wave:")) {
      ctx.setProperty(node, "value", node.value.replaceAll(":wave:", "\u{1F44B}"));
    }
  },
});

const { html } = markdownToHtml("Hi :wave:", { mdastPlugins: [emojis] });
```

Return a node from the visitor to replace the visited one. This lets you swap one type for another:

```js
const unwrapImages = defineMdastPlugin({
  name: "unwrap-images",
  paragraph(node) {
    const child = node.children[0];
    if (node.children.length === 1 && child?.type === "image") {
      return child;
    }
  },
});
```

## HAST plugins

HAST visitors take a filter so the visitor only runs for the tags you list. The filter is an array of tag names.

```js
import { markdownToHtml, defineHastPlugin } from "satteri";

const externalLinks = defineHastPlugin({
  name: "external-links",
  element: {
    filter: ["a"],
    visit(node, ctx) {
      const href = node.properties.href;
      if (typeof href === "string" && href.startsWith("http")) {
        ctx.setProperty(node, "target", "_blank");
        ctx.setProperty(node, "rel", "noopener noreferrer");
      }
    },
  },
});

const { html } = markdownToHtml(source, { hastPlugins: [externalLinks] });
```

`ctx.textContent(node)` walks the subtree and concatenates text, which is what you want for generating heading IDs.

See the [Plugin API](/docs/plugin-api/) reference for the full list of `ctx` methods.

## Restructuring siblings

Some transforms act on a node's siblings: wrapping a heading together with the content that follows it, or reordering a list. The visitor only receives the node itself, so climb to the parent with `ctx.parent()` and rewrite its child list once:

```js
const sectionize = () => {
  const done = new WeakSet();
  return defineMdastPlugin({
    name: "sectionize",
    heading(node, ctx) {
      const parent = ctx.parent(node);
      if (!parent || !("children" in parent) || done.has(parent)) return;
      done.add(parent);
      ctx.setProperty(parent, "children", groupIntoSections(parent.children));
    },
  });
};
```

Children you carry over into the new list stay the same nodes, so edits other visitors make to them still land; only the new wrapper nodes are created fresh. The first matching child does the work, and the `WeakSet` makes later matches in the same parent no-ops, since `parent()` returns the same object for every child of that parent.

For edits relative to the visited node's position, use `ctx.indexOf(node)`: `ctx.insertChildAt(parent, ctx.indexOf(node) + 1, newNode)` inserts directly after the visited node.

## Plugin order

Plugins run in array order, MDAST stage first, then HAST. Each plugin sees the tree as left by the previous one.

```js
markdownToHtml(source, {
  mdastPlugins: [emojis, unwrapImages],
  hastPlugins: [externalLinks, headingIds],
});
```

If you need to share state between visits (e.g. collecting a table of contents), close over a variable in the surrounding scope and read it back after `markdownToHtml` returns.
