---
title: "Plugin API"
description: "Visitor shapes, supported node types, and the mutation context passed to every plugin."
section: "reference"
order: 10
---

See [Plugins](/docs/plugins/) for a walkthrough.

## Plugin definition

Wrap a plugin with `defineMdastPlugin` or `defineHastPlugin` for type inference on its visitors. Both return the plugin unchanged.

A plugin is an object with a `name` and one visitor per node type you want to handle:

```js
const plugin = defineMdastPlugin({
  name: "my-plugin",
  heading(node, ctx) {
    /* ... */
  },
  link(node, ctx) {
    /* ... */
  },
});
```

### Passing plugins

`mdastPlugins` and `hastPlugins` accept either a plugin definition or a factory that returns one. Use a factory when the plugin closes over per-document state.

```ts
type MdastPluginInput = MdastPluginDefinition | (() => MdastPluginDefinition);
type HastPluginInput = HastPluginDefinition | (() => HastPluginDefinition);
```

Factories are called once per invocation, so closures reset between documents.

### Source positions

Visitors read `node.position` (the `{ start, end }` source range) only when the plugin opts in with `options: { position: true }`. Tracking positions adds a measurable parsing cost (~15% of parse), so it is off by default. `node.position` is `undefined` unless some plugin in the pipeline requests it.

```js
const plugin = defineMdastPlugin({
  name: "needs-source-range",
  options: { position: true },
  heading(node) {
    console.log(node.position); // { start, end } instead of undefined
  },
});
```

A single opted-in plugin enables positions for the whole pipeline, so a later plugin sees them too.

## MDAST visitors

An MDAST plugin maps node types to visitor functions. Each visitor receives the node (as `Readonly`) and a `ctx` object.

```ts
type MdastVisitor<N> = (node: Readonly<N>, ctx: MdastVisitorContext) => MdastVisitorResult | Promise<MdastVisitorResult>;

type MdastVisitorResult =
  | MdastNode // replace with this node
  | { raw: string; mdxExpressions?: boolean } // splice in a string, re-parsed as Markdown
  | { rawHtml: string } // deprecated — see below
  | undefined
  | null
  | void; // keep node, apply ctx mutations
```

To inject HTML, return `{ raw: "<span>…</span>", mdxExpressions: false }` rather than an mdast `html` node (`{ type: "html", value }`) — the latter renders in Markdown but throws under MDX. See [Return value semantics](#return-value-semantics).

### Supported visitor keys

Keys without a feature note are always available. Feature-gated keys only fire when the corresponding flag is enabled in `features`.

| Key                  | Feature          |
| -------------------- | ---------------- |
| `paragraph`          | —                |
| `heading`            | —                |
| `thematicBreak`      | —                |
| `blockquote`         | —                |
| `list`               | —                |
| `listItem`           | —                |
| `html`               | —                |
| `code`               | —                |
| `definition`         | —                |
| `text`               | —                |
| `emphasis`           | —                |
| `strong`             | —                |
| `inlineCode`         | —                |
| `break`              | —                |
| `link`               | —                |
| `image`              | —                |
| `linkReference`      | —                |
| `imageReference`     | —                |
| `table`              | `gfm`            |
| `tableRow`           | `gfm`            |
| `tableCell`          | `gfm`            |
| `delete`             | `gfm`            |
| `footnoteDefinition` | `gfm`            |
| `footnoteReference`  | `gfm`            |
| `math`               | `math`           |
| `inlineMath`         | `math`           |
| `yaml`               | `frontmatter`    |
| `toml`               | `frontmatter`    |
| `containerDirective` | `directive`      |
| `leafDirective`      | `directive`      |
| `textDirective`      | `directive`      |
| `superscript`        | `superscript`    |
| `subscript`          | `subscript`      |
| `descriptionList`    | `definitionList` |
| `descriptionTerm`    | `definitionList` |
| `descriptionDetails` | `definitionList` |
| `mdxJsxFlowElement`  | MDX entry        |
| `mdxJsxTextElement`  | MDX entry        |
| `mdxFlowExpression`  | MDX entry        |
| `mdxTextExpression`  | MDX entry        |
| `mdxjsEsm`           | MDX entry        |

MDX visitor keys only fire when the document is compiled via the MDX entry point (`mdxToJs` or `.mdx` imports), not from `markdownToHtml`.

## HAST visitors

HAST plugins come in two shapes depending on the node type.

### Filtered visitors

`element` and MDX JSX nodes carry a tag/component name, so their visitors take an explicit filter and only run for matching nodes.

```ts
type HastFilteredVisitor<N> = {
  filter: string[];
  visit(node: Readonly<N>, ctx: HastVisitorContext): HastNode | void | Promise<HastNode | void>;
};
```

`filter` is required. The filter is matched against `element.tagName` for `element` and against `name` for MDX JSX nodes (case-sensitive). An empty filter (`filter: []`) matches every node of that type — handy for sweeping passes, but it can get expensive on large documents, so name tags when you can.

To register multiple filtered visitors for the same node type, pass an array:

```ts
const plugin = defineHastPlugin({
  name: "headings-and-links",
  element: [
    {
      filter: ["h1", "h2", "h3"],
      visit(node, ctx) {
        /* headings */
      },
    },
    {
      filter: ["a"],
      visit(node, ctx) {
        /* links */
      },
    },
  ],
});
```

| Key                 | Filtered on  |
| ------------------- | ------------ |
| `element`           | `tagName`    |
| `mdxJsxFlowElement` | `name` (JSX) |
| `mdxJsxTextElement` | `name` (JSX) |

### Bare visitors

Leaf and value nodes don't carry a name, so they take a plain function that fires for every node of that type.

```ts
type HastVisitor<N> = (node: Readonly<N>, ctx: HastVisitorContext) => HastNode | void | Promise<HastNode | void>;
```

| Key                 | Notes                           |
| ------------------- | ------------------------------- |
| `text`              | —                               |
| `comment`           | —                               |
| `raw`               | Pass-through HTML chunks        |
| `doctype`           | —                               |
| `mdxFlowExpression` | Has `.parseExpression()` helper |
| `mdxTextExpression` | Has `.parseExpression()` helper |
| `mdxjsEsm`          | Has `.parseExpression()` helper |

### MDX expression helper

MDX expression and ESM nodes get a `parseExpression()` method attached that returns the value parsed as an ESTree `Program`, or `null` if the value is missing.

```js
mdxFlowExpression(node) {
  const tree = node.parseExpression();
  // tree is an ESTree Program
},
```

## Node lifetime

In order to avoid very expensive serialization costs between Rust and JS, Sätteri keeps both mdast and hast trees exclusively in Rust, exposing nodes to JavaScript plugins only as thin references when possible.

This means that ergonomics are slightly different than one might expect from a plain JavaScript tree, and understanding of reference vs copy semantics is important to avoid bugs.

A node kept past its visitor pass reads as the tree looked _during that pass_: later plugins' mutations are never reflected in it. Reads on a retained node keep working as long as its pass's snapshot is recoverable, which is the case when node content was resolved from the tree during the pass (this pins the pass snapshot for every node handed out in it), or when nothing has mutated or freed the tree yet by the time of the first read. Resolving means reading a child node's field or calling `ctx.parent()`/`ctx.indexOf()`; eagerly decoded fields such as an element's `tagName` or `properties` come with the node and do not pin anything. The one unrecoverable case throws: a node whose content was never resolved in-pass, first read after the tree has changed or the pipeline has ended. The error says exactly that.

Node objects are shared, not copied: the same underlying node reached through any path (`children`, `ctx.parent()`, a later pass over an unchanged tree) is the same JavaScript object, and it is frozen: assigning to its fields, `position`, `properties`, or `attributes` throws a `TypeError` rather than corrupting what later plugins see. Go through the context methods for changes, or copy first (`structuredClone(node)`) and edit the copy.

Retaining a node keeps its whole pass snapshot alive in memory until the node is garbage collected. To keep just a node's data beyond the visit, prefer an explicit copy of it and its subtree. For example, to collect all headings in a document:

```js
const headings = [];

defineHastPlugin({
  name: "collect-headings",
  element: {
    filter: ["h1", "h2"],
    visit(node) {
      headings.push(structuredClone(node));
    },
  },
});
```

Use `structuredClone(node)` for a deep, fully independent copy of the node and its subtree, or `{ ...node }` for a cheaper shallow copy when you only need this node's own fields.

To get a plain JavaScript tree of the whole document, use [`markdownToMdast` or `markdownToHast`](/docs/entry-points/#trees-without-compiling):

```js
import { markdownToMdast } from "satteri";

const tree = markdownToMdast(source); // plain objects, yours to keep
```

Note that keeping nodes in Rust is one of Sätteri's main performance advantages: the more data you copy into JavaScript, the more expensive your plugin becomes.

## Mutation context

MDAST and HAST contexts share the same shape (with small differences in `setProperty` and `textContent`). Mutations are buffered and applied after the visit completes, so it's safe to mutate while iterating.

Mutate through the context, not the node. A node is a read-only view over the Rust-side tree, so a direct write like `node.depth = 2` has no effect (and is a TypeScript error). Go through the context instead:

```ts
heading(node, ctx) {
  // node.depth = 2;                 // ignored
  ctx.setProperty(node, "depth", 2); // do this
}
```

### Properties

| Property | Type | Notes |
| --- | --- | --- |
| `source` | `string` | Original markdown source. |
| `fileURL` | `URL \| undefined` | URL of the document being processed, or `undefined` when none given. |
| `data` | `Data` | Document-scoped data bag shared across every plugin in the pipeline. Survives the mdast→hast boundary. Returned to the caller as `result.data`. Kept on the JS side, so any value is allowed (functions, class instances, etc.). |
| `sourceFormat` | `"markdown" \| "mdx"` | Which kind of file the plugin is currently running on. |

Keys on `data` are typed as `unknown` by default. Register a key's type by augmenting `DataMap`:

```ts
declare module "satteri" {
  interface DataMap {
    headings: string[];
  }
}
```

### Tree mutation

| Method                                  | Effect                                                  |
| --------------------------------------- | ------------------------------------------------------- |
| `removeNode(node)`                      | Drop the node from its parent                           |
| `replaceNode(node, newNode)`            | Swap the node for a different one, or for several       |
| `insertBefore(node, newNode)`           | Insert a sibling before the node                        |
| `insertAfter(node, newNode)`            | Insert a sibling after the node                         |
| `wrapNode(node, parentNode)`            | Wrap the node in `parentNode` (becomes its first child) |
| `prependChild(node, childNode)`         | Insert `childNode` as the first child of `node`         |
| `appendChild(node, childNode)`          | Insert `childNode` as the last child of `node`          |
| `insertChildAt(node, index, childNode)` | Insert `childNode` as the `index`-th child of `node`    |
| `removeChildAt(node, index)`            | Remove the `index`-th child of `node`                   |
| `setProperty(node, key, value)`         | Replace one field on the node                           |

`wrapNode` places the wrapped node as `parentNode`'s **first** child. If `parentNode` declares its own children, they are kept after it. Wrapping a heading in a `<div>` that holds an anchor link yields `<div><h2>…</h2><a>…</a></div>`. To put the node at an arbitrary position instead, return a replacement from the visitor.

`replaceNode`, `insertBefore`, `insertAfter`, `prependChild`, `appendChild`, and `insertChildAt` each accept either a single node or an array of nodes. An array is inserted in order at the target position, so `replaceNode(node, [a, b])` leaves `a` and `b` where `node` was. Passing `replaceNode` an empty array removes the node.

For MDAST, `key` must be a field of the node type and `value` must match that field's type. For HAST, `key` is a `string` and `value` is `unknown`.

For HAST elements, `setProperty` takes a HAST property key (e.g. `"className"`, `"href"`). For MDX JSX nodes (`mdxJsxFlowElement` / `mdxJsxTextElement`), it sets the named JSX attribute on the `attributes` array.

### Inspection

| Method                                | Effect                                                                                             |
| ------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `textContent(node, options?)` (MDAST) | Concatenated text of the subtree. Options: `{ includeImageAlt?: boolean, includeHtml?: boolean }`. |
| `textContent(node)` (HAST)            | Concatenated text of the subtree. Mirrors DOM `textContent`.                                       |
| `parent(node)`                        | The node's parent, or `undefined` at the root.                                                     |
| `indexOf(node)`                       | Index of the node in its parent's children, or `undefined` at the root.                            |

### Diagnostics

| Method                                  | Effect                                                                                                    |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `report({ message, node?, severity? })` | Push a diagnostic. `severity` defaults to `"error"`; allowed values are `"error" \| "warning" \| "info"`. |
| `getDiagnostics()`                      | Return all diagnostics collected so far.                                                                  |

`report` doesn't abort the plugin; diagnostics are collected and returned with the compile result.

## Return value semantics

| Returned                                | MDAST                                   | HAST    |
| --------------------------------------- | --------------------------------------- | ------- |
| `undefined` / `null` / `void`           | Keep node, apply `ctx` mutations        | Same    |
| The same node object                    | Same (no-op replace)                    | Same    |
| A different node                        | Replace the visited node                | Replace |
| `{ raw: string }`                       | Splice a string, re-parsed as Markdown  | N/A     |
| `{ raw: string, mdxExpressions: false }`| Same, but keep MDX `{…}` literal        | N/A     |

`{ raw }` takes a string and re-parses it as Markdown, splicing the result in place of the node. Any HTML in that string is passed through, so this is also how you inject HTML.

The `mdxExpressions` option (default `true`) controls how MDX curly braces in the string are treated. With the default, `{…}` is a live MDX expression. Set `mdxExpressions: false` to keep `{` and `}` as **literal text** — necessary when you inject generated HTML whose braces are not expressions, e.g. a Mermaid decision node `C{JWT valid?}` or KaTeX/Shiki output. In plain Markdown output the option has no effect (there are no MDX expressions), so `{ raw }` and `{ raw, mdxExpressions: false }` are identical there.

:::caution[Inject HTML with `{ raw, mdxExpressions: false }`, not an `html` node]
To splice HTML into the output, return `{ raw: "<span>…</span>", mdxExpressions: false }` — **not** an mdast `html` node such as `{ type: "html", value: "<span>…</span>" }`.

An `html` node is opaque, unparsed HTML. It renders verbatim in Markdown/HTML output, but JSX has no way to represent an unparsed HTML string, so compiling one to **MDX throws** (`raw-html`). `{ raw }` avoids this: it is re-parsed into real elements, so it works in both Markdown and MDX.

This matters most for plugins that generate markup — syntax highlighters, math renderers (KaTeX), diagram tools. If such a plugin returns `{ type: "html" }`, it will appear to work in Markdown but break under MDX.
:::

:::note[`rawHtml` is deprecated]
`{ rawHtml: x }` is equivalent to `{ raw: x, mdxExpressions: false }` and remains supported for backwards compatibility, but is deprecated — prefer the `raw` form. The old name implied HTML parsing; in reality both spellings re-parse the string as Markdown, and the only difference is whether MDX `{…}` are kept literal.
:::

## Async plugins

Any visitor may return a `Promise`. Sync and async visitors can be mixed freely. If any visitor in the pipeline is async, `markdownToHtml` and `mdxToJs` return a `Promise`; otherwise they return synchronously.

For performance, prefer sync visitors where you can: awaiting per match adds up, especially for a visitor that matches many nodes.

## Execution order

Plugins run in array order. MDAST plugins run first against the parsed MDAST tree. Sätteri then converts to HAST and runs the HAST plugins. Each plugin sees the tree as left by the previous one.

To share state across visits within a document, close over a variable in the surrounding scope. To reset that state between documents, pass a factory instead of a definition.

## How transforms compose

Each Sätteri plugin walks the tree **once** — there is no re-walking until the tree stops changing. Within that single pass:

- **Passed-through children keep their identity.** When a visitor returns a replacement that reuses the original children (e.g. `{ ...node, children: [...node.children] }`), those children are spliced back unchanged, so a transform queued on a nested one in the same pass still applies. This is what lets a single `containerDirective` visitor turn both an outer `:::note` and a nested `:::tip` into asides in one go.
- **A plugin's own freshly-built nodes are not re-walked by that plugin.** A brand-new node a visitor returns isn't visited again by the same plugin. Produce its final shape directly, or hand it to a later plugin — every plugin runs over the fully materialized output of the ones before it.
- **Dropping a subtree drops the transforms queued inside it.** If one visitor removes or replaces a node while another queued a transform on something inside that subtree, the orphaned transform is dropped and a warning is logged. Usually that's intended; the warning catches the cases where it isn't.
- **Nodes from another document throw.** Handing a context method a node kept from a previous compile — or an mdast node inside a hast plugin — fails the compile. Keep nodes around within a document freely; don't carry them across.
- **A few contradictory combinations throw.** Replacing a node with new content that reuses that same node while another plugin edits something inside it in the same pass, two replacements that each reuse the other's node, and inserting a sibling next to the root. Replacing, removing, or wrapping the root itself — say, via `ctx.parent()` on a top-level node — works fine.
