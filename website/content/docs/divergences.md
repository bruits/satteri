---
title: "Divergences"
section: "reference"
order: 50
---

Sätteri aims to match `remark`, `@mdx-js/mdx`, and the wider `unified` ecosystem in the ASTs and output it produces.

Typically, differences are unwanted and are bugs to be fixed. However, in certain cases the differences might be more beneficial. For example, `remark` might have some sort of old quirk or a bug that wasn't found, or couldn't be fixed easily for some reason. In such cases, Sätteri might choose to diverge from the reference behaviour.

## AST

### Unclosed frontmatter delimiters

When `remark-frontmatter` sees `---` or `+++` at line 1 and can't find a matching close, it suppresses list and blockquote detection for the rest of the document. Sätteri doesn't.

```markdown
---

- this is a list, not paragraph text
```

| Parser                                | Output                                |
| ------------------------------------- | ------------------------------------- |
| `remark-parse` + `remark-frontmatter` | thematicBreak + paragraph(`- this …`) |
| Sätteri (with frontmatter feature on) | thematicBreak + list                  |

## Rendering

### Code block `data.lang`

Sätteri keeps the fenced-code info-string language on the HAST element as `data.lang`. remark-rehype drops it, presumably on the grounds that it's already encoded as `properties.className` (ex, `language-rust`).

````markdown
```rust title=foo.rs
fn main() {}
```
````

| Parser        | HAST `data`                              |
| ------------- | ---------------------------------------- |
| remark-rehype | `{ meta: "title=foo.rs" }`               |
| Sätteri       | `{ lang: "rust", meta: "title=foo.rs" }` |

Both still emit `class="language-rust"` on the `<code>` element, so syntax-highlighting plugins that read `properties.className` are unaffected. Plugins that want the raw language without parsing it back out of the class name can read `data.lang` directly.

### Unknown directives in HAST

Sätteri drops `containerDirective`, `leafDirective`, and `textDirective` nodes when converting mdast to hast unless the node has `data.hName` set by a plugin. `mdast-util-to-hast`'s `defaultUnknownHandler` instead wraps unknown nodes in a `<div>` and recurses into their children.

```markdown
:::tip[Title] content :::
```

| Pipeline           | HTML                                    |
| ------------------ | --------------------------------------- |
| `remark-directive` | `<div><p>Title</p><p>content</p></div>` |
| Sätteri            | _(empty, node dropped)_                 |

Generic directives without a handler aren't meant to render anything meaningful (`remark-directive`'s own README says "Doesn't handle the directives: create your own plugin to do that"), and the `<div>` wrapper discards the directive's `name`, so the resulting HTML is no more useful than dropping the node. Plugins that _do_ set `data.hName` work identically on both sides.

### Table cell alignment

GFM tables with column alignment produce different HAST properties.

```markdown
| right |
| ----: |
|     1 |
```

| Parser        | HAST output                                |
| ------------- | ------------------------------------------ |
| Sätteri       | `<th style="text-align: right">right</th>` |
| remark-rehype | `<th align="right">right</th>`             |

The HTML renders identically. `align` is deprecated in HTML5 and `style` is the modern equivalent, so Sätteri emits `style`. A HAST plugin that reads `properties.align` won't find anything; read `properties.style` or normalize at the boundary.

### Smart punctuation pairing across nodes

With the `smartPunctuation` feature on, Sätteri converts straight quotes to typographic quotes, based on [Smartypants](https://daringfireball.net/projects/smartypants/).

`remark-smartypants` processes mdast text node on its own. An inline node between two quotes puts them in separate text nodes, so `remark-smartypants` never pairs them and leaves them straight. Sätteri, on the other hand, looks for pairs of quotes across inline nodes and converts them to curly quotes.

```markdown
a "_quoted_" word
```

| Pipeline                          | Output              |
| --------------------------------- | ------------------- |
| `remark-smartypants`              | `a "*quoted*" word` |
| Sätteri (with `smartPunctuation`) | `a “*quoted*” word` |

## MDX

### oxc vs acorn differences

Sätteri parses MDX expressions with `oxc`; `@mdx-js/mdx` uses `acorn`. The two disagree on some edge cases. We treat `oxc`'s behaviour as correct and don't consider these differences bugs.
