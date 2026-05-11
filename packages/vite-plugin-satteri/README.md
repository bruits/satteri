# vite-plugin-satteri

Vite plugin for [Sätteri](https://github.com/bruits/satteri). Import `.md`
and `.mdx` files directly. `.md` resolves to rendered HTML; `.mdx` resolves
to a JSX component.

## Install

```sh
npm install --save-dev vite-plugin-satteri satteri
yarn add -D vite-plugin-satteri satteri
pnpm add -D vite-plugin-satteri satteri
```

## Usage

```ts
// vite.config.ts
import { defineConfig } from "vite";
import satteri from "vite-plugin-satteri";

export default defineConfig({
  plugins: [
    satteri({
      features: {
        gfm: true,
        frontmatter: true,
      },
    }),
  ],
});
```

### Importing Markdown

```ts
import postHtml, { frontmatter } from "./post.md";

document.getElementById("post").innerHTML = postHtml;
console.log(frontmatter.title);
```

`default` and the named `html` export point at the same string.

### Importing MDX

The JSX runtime follows `mdx.jsxImportSource`:

```ts
// vite.config.ts
satteri({
  mdx: {
    jsxImportSource: "preact",
  },
});
```

```ts
import { render } from "preact";
import Intro, { frontmatter } from "./intro.mdx";

render(<Intro />, document.getElementById("root"));
```

In `vite serve` the MDX compile runs with `development: true`; in `vite
build` it switches to production. Override with `mdx: { development: false }`.

## Options

| Option         | Type                    | Default | Effect                                                         |
| -------------- | ----------------------- | ------- | -------------------------------------------------------------- |
| `markdown`     | `boolean`               | `true`  | Process `.md` files.                                           |
| `mdx`          | `boolean \| MdxOptions` | `true`  | Process `.mdx` files. Pass an object to configure the compile. |
| `mdastPlugins` | `MdastPluginInput[]`    | —       | MDAST-stage plugins, shared across `.md` and `.mdx`.           |
| `hastPlugins`  | `HastPluginInput[]`     | —       | HAST-stage plugins, shared across `.md` and `.mdx`.            |
| `features`     | `Features`              | —       | Parser toggles (`gfm`, `frontmatter`, `math`, …).              |

`MdxOptions` mirrors Sätteri's MDX options minus `outputFormat`. The
plugin always emits an ES module so Vite can import it.

See the [Sätteri plugins guide](https://github.com/bruits/satteri/blob/main/website/content/docs/plugins.md)
for how to write `mdastPlugins` / `hastPlugins`.

## TypeScript

Add a declaration file so TypeScript knows what `.md` / `.mdx` imports
resolve to:

```ts
declare module "*.md" {
  const html: string;
  const frontmatter: Record<string, unknown>;
  export default html;
  export { html, frontmatter };
}

declare module "*.mdx" {
  import type { ComponentType } from "preact";
  const MDXContent: ComponentType<Record<string, unknown>>;
  export const frontmatter: Record<string, unknown>;
  export default MDXContent;
}
```

Swap `preact` for your framework of choice.

## Docs

Full guide:
[website/content/docs/vite.md](https://github.com/bruits/satteri/blob/main/website/content/docs/vite.md).

## License

MIT
