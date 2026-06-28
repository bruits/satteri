<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./.github/assets/logo_light.svg" />
  <img alt="Sätteri" src="./.github/assets/logo.svg" />
</picture>

> Here the page is set, the loose locked fast, the words marked down.

High-performance Markdown and MDX processing. Parses and compiles in Rust, runs your plugins in JavaScript.

Check out the [documentation](https://satteri.bruits.org/docs/) for [installation instructions](https://satteri.bruits.org/docs/installation/), the [API reference](https://satteri.bruits.org/docs/entry-points/), and [usage examples](https://satteri.bruits.org/docs/quick-start/), try it online on the [playground](https://satteri.bruits.org/playground), or join us on [Discord](https://discord.com/invite/84pd4QtmzA)!

## Packages

Sätteri is a Rust + TypeScript monorepo containing the following Rust crates:

| Name                     | Description                                                            | Registry                                                                                                                                                                        | README                                              |
| ------------------------ | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `satteri`                | High-level Rust API for the pipeline: parse, convert, compile          | <a href="https://crates.io/crates/satteri"><img alt="Satteri Crates.io Version" src="https://img.shields.io/crates/v/satteri"></a>                                              | [README](./crates/satteri/README.md)                |
| `satteri-arena`          | Arena allocator and binary buffer primitives                           | <a href="https://crates.io/crates/satteri-arena"><img alt="Satteri Arena Crates.io Version" src="https://img.shields.io/crates/v/satteri-arena"></a>                            | [README](./crates/satteri-arena/README.md)          |
| `satteri-ast`            | MDAST and HAST node types, codecs, tree operations, and conversion     | <a href="https://crates.io/crates/satteri-ast"><img alt="Satteri AST Crates.io Version" src="https://img.shields.io/crates/v/satteri-ast"></a>                                  | [README](./crates/satteri-ast/README.md)            |
| `satteri-plugin-api`     | Rust `Plugin` trait for Rust plugins, typed visitors, and runner       | <a href="https://crates.io/crates/satteri-plugin-api"><img alt="Satteri Plugin API Crates.io Version" src="https://img.shields.io/crates/v/satteri-plugin-api"></a>             | [README](./crates/satteri-plugin-api/README.md)     |
| `satteri-napi-binding`   | NAPI bindings exposing the Rust pipeline to JavaScript                 | —                                                                                                                                                                               | [README](./crates/satteri-napi-binding/README.md)   |
| `satteri-mdxjs-rs`       | MDX-to-JavaScript compiler, fork of [mdxjs-rs] adapted for OXC         | <a href="https://crates.io/crates/satteri-mdxjs"><img alt="Satteri MDXJS Crates.io Version" src="https://img.shields.io/crates/v/satteri-mdxjs"></a>                            | [README](./crates/satteri-mdxjs-rs/readme.md)       |
| `satteri-pulldown-cmark` | CommonMark parser with MDX extension support, fork of [pulldown-cmark] | <a href="https://crates.io/crates/satteri-pulldown-cmark"><img alt="Satteri pulldown-cmark Crates.io Version" src="https://img.shields.io/crates/v/satteri-pulldown-cmark"></a> | [README](./crates/satteri-pulldown-cmark/README.md) |

And the following npm packages:

| Name                                             | Description                                          | Registry                                                                                                                                                                       | README                                                 |
| ------------------------------------------------ | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------ |
| [`satteri`][npm]                                 | TypeScript layer: plugin API and top-level functions | <a href="https://www.npmjs.com/package/satteri"><img alt="Satteri npm Version" src="https://img.shields.io/npm/v/satteri"></a>                                                 | [README](./packages/satteri/README.md)                 |
| [`satteri-expressive-code`][npm-expressive-code] | HAST plugin: render code blocks with Expressive Code | <a href="https://www.npmjs.com/package/satteri-expressive-code"><img alt="Satteri Expressive Code npm Version" src="https://img.shields.io/npm/v/satteri-expressive-code"></a> | [README](./packages/satteri-expressive-code/README.md) |
| [`vite-plugin-satteri`][npm-vite-plugin]         | Vite plugin: import `.md` and `.mdx` files           | <a href="https://www.npmjs.com/package/vite-plugin-satteri"><img alt="vite-plugin-satteri npm Version" src="https://img.shields.io/npm/v/vite-plugin-satteri"></a>             | [README](./packages/vite-plugin-satteri/README.md)     |

## Acknowledgements

Sätteri builds on the work and knowledge of several open-source projects:

- [unifiedjs] -- the ecosystem of tools for processing content with syntax trees, including [remark](https://github.com/remarkjs/remark) and [rehype](https://github.com/rehypejs/rehype)
- [pulldown-cmark] -- CommonMark pull parser
- [mdxjs-rs] -- MDX compiler by Titus Wormer, forked to use pulldown-cmark and OXC

Special thanks to the following projects for paving the way for high-performance Rust <-> JavaScript interop:

- [oxc] -- Rust JavaScript parser and compiler by the OXC team, used for MDX compilation
- [Lightning CSS] -- Rust CSS parser with a optimized JavaScript Visitor API

Sätteri is an open-source project born from [Bruits](https://bruits.org/), a Rust-focused collective 💛

[unifiedjs]: https://unifiedjs.com/
[pulldown-cmark]: https://github.com/pulldown-cmark/pulldown-cmark
[oxc]: https://oxc.rs
[Lightning CSS]: https://lightningcss.dev
[mdxjs-rs]: https://github.com/wooorm/mdxjs-rs
[npm]: https://www.npmjs.com/package/satteri
[npm-expressive-code]: https://www.npmjs.com/package/satteri-expressive-code
[npm-vite-plugin]: https://www.npmjs.com/package/vite-plugin-satteri
