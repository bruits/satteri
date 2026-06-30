# satteri

Native-enhanced Markdown parsing and processing for JavaScript. Parse and compile in Rust, create flexible plugins in JavaScript.

Check out the [documentation](https://satteri.bruits.org/docs/) for [installation instructions](https://satteri.bruits.org/docs/installation/), the [API reference](https://satteri.bruits.org/docs/entry-points/), and [usage examples](https://satteri.bruits.org/docs/quick-start/), try it online on the [playground](https://satteri.bruits.org/playground), or join us on [Discord](https://discord.com/invite/84pd4QtmzA)!

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

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
