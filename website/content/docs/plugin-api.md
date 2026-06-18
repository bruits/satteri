---
title: "Plugin API"
description: "Visitor shapes, supported node types, and the mutation context passed to every plugin."
section: "reference"
order: 10
---

See [Plugins](/docs/plugins/) for a walkthrough.

## Plugin definition

Wrap a plugin with `defineMdastPlugin` or `defineHastPlugin` for type
inference on its visitors. Both return the plugin unchanged.

A plugin is an object with a `name` and one visitor per node type you
want to handle:

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

`mdastPlugins` and `hastPlugins` accept either a plugin definition or a
factory that returns one. Use a factory when the plugin closes over
per-document state.

```ts
type MdastPluginInput = MdastPluginDefinition | (() => MdastPluginDefinition);
type HastPluginInput = HastPluginDefinition | (() => HastPluginDefinition);
```

Factories are called once per invocation, so closures reset between documents.

## MDAST visitors

An MDAST plugin maps node types to visitor functions. Each visitor
receives the node (as `Readonly`) and a `ctx` object.

```ts
type MdastVisitor<N> = (
  node: Readonly<N>,
  ctx: MdastVisitorContext,
) => MdastVisitorResult | Promise<MdastVisitorResult>;

type MdastVisitorResult =
  | MdastNode // replace with this node
  | { raw: string } // splice in raw Markdown (re-parsed)
  | { rawHtml: string } // splice in raw HTML (passed through)
  | undefined
  | null
  | void; // keep node, apply ctx mutations
```

### Supported visitor keys

Keys without a feature note are always available. Feature-gated keys
only fire when the corresponding flag is enabled in `features`.

| Key                  | Feature       |
| -------------------- | ------------- |
| `paragraph`          | —             |
| `heading`            | —             |
| `thematicBreak`      | —             |
| `blockquote`         | —             |
| `list`               | —             |
| `listItem`           | —             |
| `html`               | —             |
| `code`               | —             |
| `definition`         | —             |
| `text`               | —             |
| `emphasis`           | —             |
| `strong`             | —             |
| `inlineCode`         | —             |
| `break`              | —             |
| `link`               | —             |
| `image`              | —             |
| `linkReference`      | —             |
| `imageReference`     | —             |
| `table`              | `gfm`         |
| `tableRow`           | `gfm`         |
| `tableCell`          | `gfm`         |
| `delete`             | `gfm`         |
| `footnoteDefinition` | `gfm`         |
| `footnoteReference`  | `gfm`         |
| `math`               | `math`        |
| `inlineMath`         | `math`        |
| `yaml`               | `frontmatter` |
| `toml`               | `frontmatter` |
| `containerDirective` | `directive`   |
| `leafDirective`      | `directive`   |
| `textDirective`      | `directive`   |
| `mdxJsxFlowElement`  | MDX entry     |
| `mdxJsxTextElement`  | MDX entry     |
| `mdxFlowExpression`  | MDX entry     |
| `mdxTextExpression`  | MDX entry     |
| `mdxjsEsm`           | MDX entry     |

MDX visitor keys only fire when the document is compiled via the MDX
entry point (`mdxToJs` or `.mdx` imports), not from `markdownToHtml`.

## HAST visitors

HAST plugins come in two shapes depending on the node type.

### Filtered visitors

`element` and MDX JSX nodes carry a tag/component name, so their
visitors take an explicit filter and only run for matching nodes.

```ts
type HastFilteredVisitor<N> = {
  filter: string[];
  visit(node: Readonly<N>, ctx: HastVisitorContext): HastNode | void | Promise<HastNode | void>;
};
```

`filter` is required. The filter is matched against `element.tagName`
for `element` and against `name` for MDX JSX nodes (case-sensitive). To
register multiple filtered visitors for the same node type, pass an
array.

| Key                 | Filtered on  |
| ------------------- | ------------ |
| `element`           | `tagName`    |
| `mdxJsxFlowElement` | `name` (JSX) |
| `mdxJsxTextElement` | `name` (JSX) |

### Bare visitors

Leaf and value nodes don't carry a name, so they take a plain function
that fires for every node of that type.

```ts
type HastVisitor<N> = (
  node: Readonly<N>,
  ctx: HastVisitorContext,
) => HastNode | void | Promise<HastNode | void>;
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

MDX expression and ESM nodes get a `parseExpression()` method attached
that returns the value parsed as an ESTree `Program`, or `null` if the
value is missing.

```js
mdxFlowExpression(node) {
  const tree = node.parseExpression();
  // tree is an ESTree Program
},
```

## Mutation context

MDAST and HAST contexts share the same shape (with small differences
in `setProperty` and `textContent`). Mutations are buffered and applied
after the visit completes, so it's safe to mutate while iterating.

### Properties

| Property   | Type     | Notes                                                                                                                                                                                                                            |
| ---------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `source`   | `string` | Original markdown source.                                                                                                                                                                                                        |
| `filename` | `string` | Filename hint, used in diagnostics.                                                                                                                                                                                              |
| `data`     | `Data`   | Document-scoped data bag shared across every plugin in the pipeline. Survives the mdast→hast boundary. Returned to the caller as `result.data`. Kept on the JS side, so any value is allowed (functions, class instances, etc.). |

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
| `replaceNode(node, newNode)`            | Swap the node for a different one                       |
| `insertBefore(node, newNode)`           | Insert a sibling before the node                        |
| `insertAfter(node, newNode)`            | Insert a sibling after the node                         |
| `wrapNode(node, parentNode)`            | Wrap the node in `parentNode` (becomes its first child) |
| `prependChild(node, childNode)`         | Insert `childNode` as the first child of `node`         |
| `appendChild(node, childNode)`          | Insert `childNode` as the last child of `node`          |
| `insertChildAt(node, index, childNode)` | Insert `childNode` as the `index`-th child of `node`    |
| `removeChildAt(node, index)`            | Remove the `index`-th child of `node`                   |
| `setProperty(node, key, value)`         | Replace one field on the node                           |

`wrapNode` places the wrapped node as `parentNode`'s **first** child. If
`parentNode` declares its own children, they are kept after it. Wrapping a
heading in a `<div>` that holds an anchor link yields
`<div><h2>…</h2><a>…</a></div>`. To put the node at an arbitrary position
instead, return a replacement from the visitor.

`insertBefore`, `insertAfter`, `prependChild`, `appendChild`, and
`insertChildAt` each accept either a single node or an array of nodes. An array
is inserted in order at the target position.

For MDAST, `key` must be a field of the node type and `value` must
match that field's type. For HAST, `key` is a `string` and `value` is
`unknown`.

For HAST elements, `setProperty` takes a HAST property key (e.g.
`"className"`, `"href"`). For MDX JSX nodes (`mdxJsxFlowElement` /
`mdxJsxTextElement`), it sets the named JSX attribute on the
`attributes` array.

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

`report` doesn't abort the plugin; diagnostics are collected and
returned with the compile result.

## Return value semantics

| Returned                      | MDAST                            | HAST    |
| ----------------------------- | -------------------------------- | ------- |
| `undefined` / `null` / `void` | Keep node, apply `ctx` mutations | Same    |
| The same node object          | Same (no-op replace)             | Same    |
| A different node              | Replace the visited node         | Replace |
| `{ raw: string }`             | Splice raw Markdown (re-parsed)  | N/A     |
| `{ rawHtml: string }`         | Splice raw HTML (passthrough)    | N/A     |

## Async plugins

Any visitor may return a `Promise`. Sync and async visitors can be
mixed freely. If any visitor in the pipeline is async, `markdownToHtml`
and `mdxToJs` return a `Promise`; otherwise they return synchronously.

## Execution order

Plugins run in array order. MDAST plugins run first against the
parsed MDAST tree. Sätteri then converts to HAST and runs the HAST
plugins. Each plugin sees the tree as left by the previous one.

To share state across visits within a document, close over a variable
in the surrounding scope. To reset that state between documents, pass
a factory instead of a definition.
