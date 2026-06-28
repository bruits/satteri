---
title: "Expressive Code"
description: "Syntax-highlight code blocks with Expressive Code in a Sätteri project."
section: "guides"
order: 30
---

`satteri-expressive-code` renders fenced code blocks with [Expressive Code](https://expressive-code.com): syntax highlighting through Shiki, editor and terminal frames, line markers, and a copy button. It's a HAST plugin — the Sätteri equivalent of `rehype-expressive-code`.

## Install

{{ install pkg="satteri-expressive-code satteri" /}}

`satteri` is a peer dependency, so install it alongside the plugin.

## Usage

Pass the plugin to `hastPlugins`. Its visitor is async, so the compile returns a `Promise`:

```js
import { markdownToHtml } from "satteri";
import expressiveCode from "satteri-expressive-code";

const { html } = await markdownToHtml(source, {
  hastPlugins: [expressiveCode({ themes: ["github-dark", "github-light"] })],
});
```

The CSS and JS that Expressive Code needs are injected into the rendered HTML once per document, so the output is self-contained.

## Options

| Option     | Type                                | Default | Effect                                                                     |
| ---------- | ----------------------------------- | ------- | -------------------------------------------------------------------------- |
| `themes`   | `(string \| ExpressiveCodeTheme)[]` | —       | Shiki theme names or theme objects. CSS variables are generated per theme. |
| `tabWidth` | `number`                            | `2`     | Spaces a tab expands to in code blocks; `0` keeps tabs.                    |

When `themes` is omitted, Expressive Code's own defaults apply (currently `github-dark` and `github-light`).

`expressiveCode` extends [`ExpressiveCodeConfig`](https://expressive-code.com), so every other Expressive Code option — `styleOverrides`, `plugins`, `useDarkModeMediaQuery`, `themeCssSelector`, and the rest — is accepted alongside the two above.

## Themes

`themes` takes any mix of:

- a bundled Shiki theme name (e.g. `github-dark`), loaded for you;
- a theme object in VS Code / Shiki JSON format;
- an `ExpressiveCodeTheme` instance.

Expressive Code generates a set of CSS variables per theme. Pass exactly one dark and one light theme and it also emits a `prefers-color-scheme` media query by default — control that with `useDarkModeMediaQuery` and `themeCssSelector`.

```js
expressiveCode({ themes: ["github-light", "github-dark"] });
```

## Advanced

Three optional hooks customise rendering. The two per-block hooks receive the block `input` and a `document` (`{ source, filename }`); all three may also return a `Promise`.

| Hook                   | Type                                           | Effect                                                              |
| ---------------------- | ---------------------------------------------- | ------------------------------------------------------------------- |
| `getBlockLocale`       | `({ input, document }) => string \| undefined` | Set a per-block locale, for multi-language sites.                   |
| `customCreateBlock`    | `({ input, document }) => ExpressiveCodeBlock` | Replace how each `ExpressiveCodeBlock` is constructed.              |
| `customCreateRenderer` | `(options) => SatteriExpressiveCodeRenderer`   | Replace the renderer. Its result is cached and reused per document. |

`customCreateRenderer` returns the same shape as the exported `createRenderer`, which builds the `ExpressiveCode` instance (loading any Shiki themes) and the assets to inject:

```ts
createRenderer(options?: SatteriExpressiveCodeOptions): Promise<SatteriExpressiveCodeRenderer>;

interface SatteriExpressiveCodeRenderer {
  ec: ExpressiveCode;
  baseStyles: string;
  themeStyles: string;
  jsModules: string[];
}
```

## Re-exports

Everything from `expressive-code` is re-exported from `satteri-expressive-code`, so its themes and helpers are importable from one place. HAST types live at the `satteri-expressive-code/hast` subpath.

See the [Plugins](/docs/plugins/) guide and the [Plugin API](/docs/plugin-api/) reference.
