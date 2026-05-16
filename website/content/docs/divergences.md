---
title: "Divergences"
section: "reference"
order: 50
---

Sätteri aims to be as compatible as possible with `remark`, `@mdx-js/mdx` and the greater `unified` ecosystem with regards to the ASTs and results generated.

Typically, differences are unwanted and are bugs to be fixed. However, in certain cases the differences might be more beneficial. For example, `remark` might have some sort of old quirk or a bug that wasn't found, or couldn't be fixed easily for some reason. In such cases, Sätteri might choose to diverge from the reference behaviour.

## AST

### Unclosed frontmatter delimiters

When `remark-frontmatter` sees `---` or `+++` at line 1 and can't find
a matching close, it suppresses list and blockquote detection for the
rest of the document. Sätteri doesn't.

```markdown
---

- this is a list, not paragraph text
```

| Parser                                | Output                                |
| ------------------------------------- | ------------------------------------- |
| `remark-parse` + `remark-frontmatter` | thematicBreak + paragraph(`- this …`) |
| Sätteri (with frontmatter feature on) | thematicBreak + list                  |

A real document either closes the frontmatter or doesn't open one. The
remark behaviour isn't specified anywhere and isn't useful.

## Rendering

### Code block `data.lang`

Sätteri keeps the fenced-code info-string language on the HAST element as
`data.lang`. remark-rehype drops it, on the grounds that it's already
encoded as `properties.className` (`language-rust`).

````markdown
```rust title=foo.rs
fn main() {}
```
````

| Parser        | HAST `data`                              |
| ------------- | ---------------------------------------- |
| remark-rehype | `{ meta: "title=foo.rs" }`               |
| Sätteri       | `{ lang: "rust", meta: "title=foo.rs" }` |

Both still emit `class="language-rust"` on the `<code>` element, so
syntax-highlighting plugins that read `properties.className` are
unaffected. Plugins that want the raw language without parsing it back
out of the class name can read `data.lang` directly.

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

The HTML renders identically. `align` is deprecated in HTML5 and
`style` is the modern equivalent, so Sätteri emits `style`. A HAST
plugin that reads `properties.align` won't find anything; read
`properties.style` or normalize at the boundary.

## MDX

### oxc vs acorn differences

Sätteri uses `oxc` to parse MDX expressions, while `@mdx-js/mdx` uses `acorn`. This can naturally lead to some differences in regard to handling particular edge cases. We generally believe that `oxc` is correct, and do not consider these differences to be bugs.
