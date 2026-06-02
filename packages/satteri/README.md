# satteri

Native-enhanced Markdown parsing and processing for JavaScript. Parse and compile in Rust, create flexible plugins in JavaScript.

## Install

```sh
npm install satteri
yarn add satteri
pnpm add satteri
```

## Usage

### Markdown to HTML

```ts
import { markdownToHtml } from "satteri";

const html = markdownToHtml("# Hello\n\nWorld");
// <h1>Hello</h1>\n<p>World</p>
```

### MDX to JS

```ts
import { mdxToJs } from "satteri";

const js = mdxToJs("# Hello\n\n<MyComponent />");
```

### With plugins

Both functions accept `mdastPlugins` (operate on the Markdown AST before conversion to HAST) and `hastPlugins` (operate on the HAST before output).

```ts
import { markdownToHtml } from "satteri";
import { removeHeadings } from "./my-mdast-plugins.js";
import { addLinkClasses } from "./my-hast-plugins.js";

const html = markdownToHtml("# Hello\n\n[link](https://example.com)", {
  mdastPlugins: [removeHeadings],
  hastPlugins: [addLinkClasses],
});
```

If you're familiar with the unified ecosystem, mdast and hast plugins would be similar to remark and rehype plugins, respectively, and re-uses the same AST shape for both. This project does not currently have an equivalent of micromark or recma plugins.

## Plugins

### MDAST plugins

MDAST plugins run on the Markdown syntax tree, allowing you to do things like replace emoji shortcodes, unwrap images from paragraphs, or collect headings for a table of contents before Markdown is transformed to HTML / JS. Define visitor methods named after node types (`heading`, `code`, `link`, `image`, etc.). Each visitor receives the node and a context object for mutations.

```ts
const emojis = defineMdastPlugin({
  name: "emojis",
  text(node, ctx) {
    if (node.value.includes(":wave:")) {
      ctx.setProperty(node, "value", node.value.replaceAll(":wave:", "\u{1F44B}"));
    }
  },
});
```

Visitors can alternatively return a replacement node, raw Markdown, or raw HTML. This is useful when the replacement can't be expressed as property changes on the original node:

```ts
const highlightCode = defineMdastPlugin({
  name: "highlight-code",
  code(node) {
    return { rawHtml: `<pre class="highlighted">${escape(node.value)}</pre>` };
  },
});
```

All standard mdast node types are supported, plus GFM extensions (`table`, `tableRow`, `tableCell`, `delete`, `footnoteDefinition`, `footnoteReference`) and MDX nodes (`mdxJsxFlowElement`, `mdxJsxTextElement`, `mdxFlowExpression`, `mdxTextExpression`, `mdxjsEsm`) if enabled. For more information on the AST shape and node types, see the [mdast spec](https://github.com/syntax-tree/mdast).

### HAST plugins

HAST plugins run on the HTML syntax tree after mdast-to-hast conversion, allowing you to do things like add classes to elements, set attributes on links, or wrap HTML elements with other elements, etc. Element visitors use a `filter` array to specify which tag names (or component names for MDX) to match.

```ts
const addLinkClasses = defineHastPlugin({
  name: "add-link-classes",
  element: {
    filter: ["a"],
    visit(node, ctx) {
      ctx.setProperty(node, "class", "link");
      ctx.setProperty(node, "target", "_blank");
    },
  },
});
```

Multiple filter groups on the same node type:

```ts
const multiFilter = defineHastPlugin({
  name: "multi-filter",
  element: [
    {
      filter: ["h1", "h2", "h3"],
      visit(node, ctx) {
        ctx.setProperty(node, "class", "heading");
      },
    },
    {
      filter: ["a"],
      visit(node, ctx) {
        ctx.setProperty(node, "target", "_blank");
      },
    },
  ],
});
```

An empty filter matches all elements, but can quickly become expensive when used on large documents, so use with caution:

```ts
const allElements = defineHastPlugin({
  name: "all-elements",
  element: {
    filter: [],
    visit(node, ctx) {
      ctx.setProperty(node, "data-visited", "true");
    },
  },
});
```

Non-element visitors (`text`, `comment`, `raw`, `doctype`, MDX expression types) use bare functions instead of filter objects:

```ts
const uppercaseText = defineHastPlugin({
  name: "uppercase-text",
  text(node, ctx) {
    ctx.setProperty(node, "value", node.value.toUpperCase());
  },
});
```

For more information on the AST shape and node types, see the [hast spec](https://github.com/syntax-tree/hast).

### Mutating nodes

Unlike remark and rehype plugins, nodes in Sätteri inside plugins are read-only. The AST lives in Rust memory and JavaScript only has a "view" over the different nodes, so direct mutations like `node.value = "new text"` have no effect. Use the context methods (`ctx.setProperty`, `ctx.removeNode`, `ctx.replaceNode`, etc.) instead, which send changes back to Rust in an efficient way.

```ts
// Won't work
heading(node, ctx) {
  node.depth = 2; // no effect, TypeScript will also complain that the node is readonly
}

// Do this instead
heading(node, ctx) {
  ctx.setProperty(node, "depth", 2);
}

// Or return a new node to replace it entirely, but this is less efficient and generally not recommended
heading(node) {
  return { ..node, depth: 2 };
}
```

### How transforms compose

Unlike remark and rehype, which re-walk the tree until it stops changing, each Sätteri plugin walks the tree **once**. Within that single pass:

- **Passed-through children keep their identity.** When a visitor returns a replacement that reuses the original node's children (e.g. `{ ...node, children: [...node.children] }`), those children are spliced back unchanged — so a transform the same pass queues on a nested one still applies. This is what lets a `containerDirective` visitor turn both an outer `:::note` and a nested `:::tip` into asides in one go.
- **A plugin's own freshly-built nodes are not re-walked by that plugin.** If a visitor returns a brand-new node (one that didn't come from the tree), the same plugin won't visit it. Produce its final shape directly, or hand it off to a later plugin — every plugin runs over the fully materialized output of the ones before it.
- **Dropping a subtree drops transforms queued inside it.** If one visitor removes or replaces a node while another (in the same pass) had queued a transform on something inside that subtree, the orphaned transform is dropped and a warning is logged. This is usually intentional — you discarded that subtree on purpose — but the warning helps catch the cases where it isn't.

### Async plugins

Visitors can optionally be async. When any visitor is async, `markdownToHtml` and `mdxToJs` return a `Promise<string>` instead of `string`. For performance reasons, it is typically best to avoid async visitors, especially if your visitor matches a large number of nodes.

````ts
const highlighter = await createHighlighter({ themes: ["github-dark"], langs: ["js", "ts"] });

const asyncHighlight = defineMdastPlugin({
  name: "async-highlight",
  async code(node) {
    const html = await highlighter.codeToHtml(node.value, {
      lang: node.lang,
      theme: "github-dark",
    });
    return { rawHtml: html };
  },
});

// Returns Promise<string> when async plugins are used
const html = await markdownToHtml("```js\ncode\n```", {
  mdastPlugins: [asyncHighlight],
});
````

## API

### `markdownToHtml(source: string, options?: CompileOptions)`

Parse Markdown and compile to HTML. Returns `string` if all plugins are sync, `Promise<string>` if any are async.

```ts
const html = markdownToHtml("# Hello\n\nWorld");
// <h1>Hello</h1>\n<p>World</p>
```

### `mdxToJs(source: string, options?: MdxCompileOptions)`

Parse MDX and compile to JavaScript module code. Same sync/async return behavior.

```ts
const js = mdxToJs("# Hello\n\n<MyComponent />");
```

#### Static optimization

The `optimizeStatic` option for MDX collapses static subtrees into pre-rendered HTML strings, reducing the number of JSX element calls in the output and increasing rendering performance. Dynamic content (JSX components, expressions) is preserved as normal JSX calls.

```ts
// Astro-style: wraps static HTML in <Fragment set:html="...">
const js = mdxToJs("# Hello\n\nWorld", {
  optimizeStatic: {
    component: "Fragment",
    prop: "set:html",
  },
});

// React-style: wraps in <div dangerouslySetInnerHTML={{ __html: "..." }}>
const js = mdxToJs("# Hello\n\nWorld", {
  optimizeStatic: {
    component: "div",
    prop: "dangerouslySetInnerHTML",
    wrapPropValue: true,
  },
});
```

The `ignoreElements` option can be used to exclude specific elements from collapsing.

Shoutout to [Bjorn Lu](https://bjornlu.com) for originally developing this optimization for [Astro](https://astro.build/).

### `markdownToMdast(source: string)`

Parse Markdown and return a complete mdast tree. This can be useful if you wanted to benefit from the fast native parsing of Sätteri, but ultimately wanted another pipeline to handle transformations and compilation, e.g. using remark plugins and `remark-stringify` to convert back to Markdown after processing.

```ts
import { markdownToMdast } from "satteri";

const tree = markdownToMdast("# Hello\n\nWorld");
// tree.children[0].type === "heading"
// tree.children[0].depth === 1
```

### `mdxToMdast(source: string)`

Parse MDX and return a complete mdast tree.

```ts
const tree = mdxToMdast('<Component foo="bar" />');
// tree.children[0].type === "mdxJsxFlowElement"
// tree.children[0].name === "Component"
```

### `markdownToHast(source: string)`

Parse Markdown, convert to hast, and return a complete hast tree.

```ts
const tree = markdownToHast("# Hello\n\nWorld");
// tree.children[0].type === "element"
// tree.children[0].tagName === "h1"
```

### `mdxToHast(source: string)`

Parse MDX, convert to hast, and return a complete hast tree.

```ts
const tree = mdxToHast("<MyComponent />");
// tree.children[0].type === "mdxJsxFlowElement"
// tree.children[0].name === "MyComponent"
```

### `defineMdastPlugin(definition: MdastPluginDefinition)`

Type-safe wrapper for MDAST plugin definitions.

### `defineHastPlugin(definition: HastPluginDefinition)`

Type-safe wrapper for HAST plugin definitions.

### `CompileOptions`

```ts
interface CompileOptions {
  mdastPlugins?: MdastPluginDefinition[];
  hastPlugins?: HastPluginDefinition[];
  features?: Features;
  fileURL?: URL;
}

// mdxToJs accepts MdxCompileOptions, which extends CompileOptions
interface MdxCompileOptions extends CompileOptions {
  optimizeStatic?: OptimizeStaticConfig;
}
```

`fileURL` is the `URL` of the document being processed, surfaced to plugins as `ctx.fileURL` (a `URL`, or `undefined` when omitted). Pass a file URL such as Astro's `fileURL`, or convert a filesystem path with Node's `pathToFileURL`; read it back with `fileURLToPath(ctx.fileURL)`.

`Features` controls the Markdown extensions Sätteri's parser recognizes: `gfm`, `frontmatter`, `math`, `directive`, `smartPunctuation`, etc. `gfm`, `math`, and `smartPunctuation` also accept granular options. For example, `math: { singleDollarTextMath: false }` keeps single `$` as literal text. See the [Features reference](https://satteri.dev/docs/features/) for the full list.

## License

MIT
