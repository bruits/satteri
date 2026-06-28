---
title: "Options"
description: "The CompileOptions shared by the entry points, plus the MDX-only options and JSX output settings."
section: "reference"
order: 4
---

Options for Sätteri's [entry points](/docs/entry-points/). `CompileOptions` is shared by `markdownToHtml` and `mdxToJs`, which additionally accepts the [MDX options](#mdx-options) below.

## CompileOptions

| Option         | Type                 | Notes                                                                       |
| -------------- | -------------------- | --------------------------------------------------------------------------- |
| `mdastPlugins` | `MdastPluginInput[]` | MDAST plugins, or factories that return one. See [Plugins](/docs/plugins/). |
| `hastPlugins`  | `HastPluginInput[]`  | HAST plugins, or factories.                                                 |
| `features`     | `Features`           | Parser extensions. See [Features](/docs/features/).                         |
| `fileURL`      | `URL`                | The document's URL, surfaced to plugins as `ctx.fileURL`.                   |
| `data`         | `Data`               | Initial [data bag](#data).                                                  |

### fileURL

`fileURL` must be a `URL`, not a string — pass an existing file URL (such as Astro's `fileURL`), or convert a filesystem path with Node's `pathToFileURL`:

```js
import { pathToFileURL } from "node:url";

markdownToHtml(source, { fileURL: pathToFileURL("docs/intro.md") });
```

Plugins read it back as `ctx.fileURL` (a `URL`, or `undefined` when omitted).

### data

`data` seeds the document-level data bag before any plugin runs. It is the **same object** plugins mutate via `ctx.data` and the caller reads back as `result.data`, so values flow both into and out of a compile:

```js
const data = { title: "Original" };
const result = markdownToHtml(source, { mdastPlugins: [rewriteTitle], data });

result.data === data; // true — the seeded object is returned, not a copy
```

It is used by reference and mutated in place, so pass a throwaway object per compile rather than a shared one. Omit it and each compile gets a fresh `{}`. The bag lives entirely on the JS side, so any value is allowed (functions, class instances, `Map`/`Set`), and references survive the mdast→hast boundary. See the [data bag](/docs/plugin-api/#mutation-context) for the plugin side and `DataMap` typing.

## MDX options

`mdxToJs` accepts everything in `CompileOptions` plus the MDX-only fields below (also exported on their own as `MdxOnlyOptions`).

### optimizeStatic

Collapses contiguous static subtrees into a single pre-rendered HTML string, cutting the number of JSX element calls in the output. Dynamic content (components, `{expressions}`) is left as normal JSX.

```js
// Astro-style: wraps static HTML in <Fragment set:html="…">
mdxToJs(source, {
  optimizeStatic: { component: "Fragment", prop: "set:html" },
});

// React-style: wraps in <div dangerouslySetInnerHTML={{ __html: "…" }}>
mdxToJs(source, {
  optimizeStatic: { component: "div", prop: "dangerouslySetInnerHTML", wrapPropValue: true },
});
```

| Field            | Type       | Notes                                                                 |
| ---------------- | ---------- | --------------------------------------------------------------------- |
| `component`      | `string`   | Element/component the static HTML is wrapped in.                      |
| `prop`           | `string`   | Prop the HTML string is passed on.                                    |
| `wrapPropValue`  | `boolean`  | When `true`, wrap the value as `{{ __html: "…" }}`. Default: `false`. |
| `ignoreElements` | `string[]` | Tag names to keep as JSX calls instead of collapsing.                 |

Elements that can be overridden at runtime via `export const components` are kept as JSX automatically, so component overrides still apply.

This optimization was originally developed by [Bjorn Lu](https://bjornlu.com) for [Astro](https://astro.build/).

### JSX output

The remaining MDX options control the generated JavaScript and are named after the standard `@mdx-js/mdx` compiler options (Sätteri's MDX compiler is a separate implementation — see [Divergences](/docs/divergences/)):

| Option                     | Type                           | Default                 | Notes                                                                                         |
| -------------------------- | ------------------------------ | ----------------------- | --------------------------------------------------------------------------------------------- |
| `jsx`                      | `boolean`                      | `false`                 | Keep JSX instead of compiling it to function calls.                                           |
| `jsxRuntime`               | `"automatic" \| "classic"`     | `"automatic"`           | JSX runtime.                                                                                  |
| `jsxImportSource`          | `string`                       | `"react"`               | Where the automatic runtime is imported from (e.g. `"preact"`).                               |
| `providerImportSource`     | `string`                       | —                       | Where the component provider is imported from.                                                |
| `development`              | `boolean`                      | `false`                 | Development mode (adds debugging info).                                                       |
| `pragma`                   | `string`                       | `"React.createElement"` | Classic-runtime JSX pragma.                                                                   |
| `pragmaFrag`               | `string`                       | `"React.Fragment"`      | Classic-runtime fragment pragma.                                                              |
| `pragmaImportSource`       | `string`                       | `"react"`               | Where the classic pragma is imported from.                                                    |
| `outputFormat`             | `"program" \| "function-body"` | `"program"`             | `program` emits an ES module; `function-body` emits a body for `new Function()` / `evaluate`. |
| `elementAttributeNameCase` | `"react" \| "html"`            | `"react"`               | Casing for attributes on rehype-produced elements.                                            |
| `stylePropertyNameCase`    | `"dom" \| "css"`               | `"dom"`                 | Casing for keys in parsed `style` objects.                                                    |
