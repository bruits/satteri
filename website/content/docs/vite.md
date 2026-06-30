---
title: "Usage with Vite"
section: "guides"
order: 20
---

`vite-plugin-satteri` lets you `import` Markdown and MDX files directly in a Vite project. `.md` imports give you the rendered HTML; `.mdx` imports give you a JSX component. YAML or TOML frontmatter comes through parsed as a named export either way.

## Install

{{ install pkg="vite-plugin-satteri satteri" dev=true /}}

`satteri` is a peer dependency, so install it alongside the plugin.

## Configure

Add the plugin to `vite.config.ts`:

```js
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

That's the whole setup. Any `.md` or `.mdx` file in your project is now importable.

## Importing Markdown

A `.md` import gives you the rendered HTML as a string plus any parsed frontmatter (YAML between `---` fences, or TOML between `+++` fences):

```js
import postHtml, { frontmatter } from "./post.md";

document.getElementById("post").innerHTML = postHtml;
console.log(frontmatter.title);
```

The default export and the named `html` export point at the same string, so pick whichever reads better at the call site.

## Importing MDX

An `.mdx` import is an ES module that exports a component. The JSX runtime follows whatever you configure in `mdx.jsxImportSource`:

```js
import { defineConfig } from "vite";
import satteri from "vite-plugin-satteri";

export default defineConfig({
  plugins: [
    satteri({
      mdx: {
        jsxImportSource: "preact",
      },
    }),
  ],
});
```

Then in your app:

```js
import { render } from "preact";
import Intro, { frontmatter } from "./intro.mdx";

render(<Intro />, document.getElementById("root"));
```

By default, the plugin compiles MDX with `development: true` in `serve` so React gives you useful component stacks, and switches to the production runtime in `build`. Override with `mdx: { development: false }`.

## Options

| Option         | Type                    | Default | Effect                                                         |
| -------------- | ----------------------- | ------- | -------------------------------------------------------------- |
| `markdown`     | `boolean`               | `true`  | Process `.md` files.                                           |
| `mdx`          | `boolean \| MdxOptions` | `true`  | Process `.mdx` files. Pass an object to configure the compile. |
| `mdastPlugins` | `MdastPluginInput[]`    | —       | MDAST-stage plugins, shared across `.md` and `.mdx`.           |
| `hastPlugins`  | `HastPluginInput[]`     | —       | HAST-stage plugins, shared across `.md` and `.mdx`.            |
| `features`     | `Features`              | —       | Parser toggles. See [Features](/docs/features/).               |

`MdxOptions` mirrors Sätteri's MDX options minus `outputFormat`. The plugin always emits an ES module so Vite can import it.

Plugins given here apply to every Markdown and MDX file. See the [Plugins](/docs/plugins/) guide for how to write them.

## TypeScript

TypeScript does not know by default what a `.md` or `.mdx` import resolves to.

Add a declaration file (e.g. `src/satteri-modules.d.ts`) in order to teach TypeScript:

```js
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

Swap `preact` for `react` (or your framework of choice) to match your `jsxImportSource`.
