# vite-plugin-satteri

Vite plugin for [Sätteri](https://github.com/bruits/satteri). Import `.md` and `.mdx` files directly: `.md` resolves to rendered HTML, `.mdx` to a JSX component.

See the [Vite guide](https://satteri.bruits.org/docs/vite/) for configuration, importing Markdown and MDX, options, and TypeScript setup.

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
  plugins: [satteri()],
});
```

Then import Markdown or MDX directly:

```ts
import postHtml, { frontmatter } from "./post.md";
```

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
