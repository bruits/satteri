---
title: "Features"
description: "Toggle and configure the Markdown extensions Sätteri's parser supports."
section: "reference"
order: 5
---

The `features` option on `markdownToHtml`, `mdxToJs`, `markdownToHast`,
and `mdxToHast` toggles which Markdown extensions the parser recognizes. By default, Sätteri enables `gfm`, `frontmatter`, `math` and `headingAttributes`.

```js
import { markdownToHtml } from "satteri";

markdownToHtml(source, {
  features: {
    gfm: true,
    frontmatter: true,
    math: true,
    headingAttributes: true,
    directive: false,
    superscript: false,
    subscript: false,
    wikilinks: false,
    smartPunctuation: false,
  },
});
```

`gfm`, `math`, and `smartPunctuation` each accept a boolean or a granular
options object. Passing the object turns the feature on.

## GFM

```ts
gfm?: boolean | {
  footnotes?: boolean | FootnoteOptions
}
```

Default: `true`. Enables tables, footnotes, strikethrough, task lists,
and GitHub-style autolinks.

### Customizing footnotes

The three strings in the footnotes section (the `<h2>` label, the
backref `aria-label`, and the backref text) are configurable without a
post-processing plugin:

```js
markdownToHtml(source, {
  features: {
    gfm: {
      footnotes: {
        label: "Notes de bas de page",
        backContent: "↑",
        backLabel: "Retour à la référence {reference}",
      },
    },
  },
});
```

`backLabel` and `backContent` each accept either a string template or a
callback.

In a string template, the `{reference}` token expands to the footnote
number on the first backref (e.g. `1`) and to `number-K` on repeated
backrefs (e.g. `1-2`). Template mode also appends a `<sup>K</sup>` marker after `backContent` on reruns.

For full control, you can pass a callback to these options:

```ts
type FootnoteBackrefCallback = (referenceNumber: number, rerunIndex: number) => string;
```

```js
markdownToHtml(source, {
  features: {
    gfm: {
      footnotes: {
        backLabel: (n, k) => (k > 1 ? `Retour ${n}-${k}` : `Retour ${n}`),
        backContent: (_n, k) => (k === 1 ? "↑" : `↑${k}`),
      },
    },
  },
});
```

Both arguments are 1-based. `referenceNumber` is the footnote number a
reader sees; `rerunIndex` is `1` for the first backref to a given
definition, `2` for the second, and so on. Callback mode skips the
auto-`<sup>K</sup>`: the callback returns the final content for each
backref.

## Math

Default: `true`.

```ts
math?: boolean | {
  singleDollarTextMath?: boolean
}
```

Parses `$$ ... $$` display math and `$ ... $` inline
math.

Set `singleDollarTextMath: false` to keep `$$ ... $$` working while
treating single dollars as literal text. Useful for prose with currency
like "from $50 to $100":

```js
markdownToHtml(source, {
  features: { math: { singleDollarTextMath: false } },
});
```

## Frontmatter

Default: `true`.

```ts
frontmatter?: boolean
```

Recognizes YAML (`--- ... ---`) and TOML (`+++ ... +++`)
blocks at the top of a document.

The parsed block is returned alongside the rendered output:

```js
const { html, frontmatter } = markdownToHtml(source);
if (frontmatter) {
  console.log(frontmatter.kind); // "yaml" or "toml"
  console.log(frontmatter.value); // raw string between the delimiters
}
```

Sätteri does not currently parse the TOML or YAML.

## Heading attributes

Default: `true`.

```ts
headingAttributes?: boolean
```

Recognizes curly-brace attribute syntax on headings:

```markdown
## My heading {#my-id .my-class}
```

The id and classes appear on the rendered heading, producing the following HTML:

```html
<h2 id="my-id" class="my-class">My heading</h2>
```

## Directives

Default: `false`.

```ts
directive?: boolean
```

Enables container (`:::name`), leaf (`::name`), and
text (`:name`) directives as defined by
[remark-directive](https://github.com/remarkjs/remark-directive). The
parser produces directive nodes. Rendering them is up to a plugin; the
default mdast→hast conversion drops them.

## Superscript / subscript

Default: `false` for both.

```ts
superscript?: boolean
subscript?: boolean
```

`^text^` becomes `<sup>text</sup>` and
`~text~` becomes `<sub>text</sub>`.

## Wikilinks

Default: `false`.

```ts
wikilinks?: boolean
```

Recognizes `[[Target]]` and `[[Target|Label]]` as
links.

## Smart punctuation

Default: `false`.

```ts
smartPunctuation?: boolean | {
  quotes?: boolean
  dashes?: boolean
  ellipses?: boolean
}
```

Pass `true` to enable all three categories at once, or
an options object to turn on just the parts you want:

```js
// Curly quotes only; leave -- and ... alone.
markdownToHtml(source, {
  features: { smartPunctuation: { quotes: true, dashes: false, ellipses: false } },
});
```

Omitted keys in the options object default to `true`, so
`{ dashes: false }` enables quotes and ellipses but disables dashes.
