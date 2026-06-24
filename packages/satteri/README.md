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

const { html } = markdownToHtml("# Hello\n\nWorld");
// <h1>Hello</h1>\n<p>World</p>
```

### MDX to JS

```ts
import { mdxToJs } from "satteri";

const { code } = mdxToJs("# Hello\n\n<MyComponent />");
```

### With plugins

Both functions accept `mdastPlugins` (operate on the Markdown AST) and `hastPlugins` (operate on the HTML AST). A plugin is an object with a `name` and a visitor per node type; `defineMdastPlugin` / `defineHastPlugin` add type inference.

```ts
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

If you're familiar with the unified ecosystem, mdast and hast plugins are similar to remark and rehype plugins, respectively, reusing the same AST shapes.

## Documentation

This README covers the basics. The full documentation covers the complete API, the plugin system, every parser feature, and framework integration:

- **[Documentation home](https://satteri.bruits.org/docs/)** — start here
- [Quick start](https://satteri.bruits.org/docs/quick-start/)
- [Compiling](https://satteri.bruits.org/docs/compile/) — functions, options, and MDX settings
- [Plugins](https://satteri.bruits.org/docs/plugins/) and [Plugin API](https://satteri.bruits.org/docs/plugin-api/)
- [Features](https://satteri.bruits.org/docs/features/) — GFM, math, directives, and more

## License

MIT
