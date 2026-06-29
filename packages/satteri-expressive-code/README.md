# satteri-expressive-code

Sätteri HAST plugin that renders fenced code blocks with [Expressive Code](https://expressive-code.com): syntax highlighting via Shiki, editor and terminal frames, line markers, and a copy button. The Sätteri equivalent of [`rehype-expressive-code`](https://expressive-code.com).

See the [Expressive Code guide](https://satteri.bruits.org/docs/expressive-code/) for themes, options, and the full API.

## Install

```sh
npm install satteri-expressive-code satteri
yarn add satteri-expressive-code satteri
pnpm add satteri-expressive-code satteri
```

`satteri` is a peer dependency.

## Usage

```ts
import { markdownToHtml } from "satteri";
import expressiveCode from "satteri-expressive-code";

const { html } = await markdownToHtml(source, {
  hastPlugins: [expressiveCode({ themes: ["github-dark", "github-light"] })],
});
```

## Development

Refer to [CONTRIBUTING.md](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for development setup and workflow details.
