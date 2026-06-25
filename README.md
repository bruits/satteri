<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./.github/assets/logo_light.svg" />
  <img alt="Sätteri" src="./.github/assets/logo.svg" />
</picture>

> Here the page is set, the loose locked fast, the words marked down.

High-performance Markdown and MDX processing. Parses and compiles in Rust, runs your plugins in JavaScript.

Check out the npm package's [documentation](./packages/satteri/README.md) for installation instructions, API reference, and usage examples, [try it online on the playground](https://satteri.bruits.org/playground), or join us on [Discord](https://discord.com/invite/84pd4QtmzA)!

## Packages

Sätteri is a Rust + TypeScript monorepo containing the following Rust crates:

| Name                     | Description                                                            | README                                              | Registry                                                                                                                                                                        |
| ------------------------ | ---------------------------------------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `satteri`                | High-level Rust API for the pipeline: parse, convert, compile          | [README](./crates/satteri/README.md)                | <a href="https://crates.io/crates/satteri"><img alt="Satteri Crates.io Version" src="https://img.shields.io/crates/v/satteri"></a>                                              |
| `satteri-arena`          | Arena allocator and binary buffer primitives                           | [README](./crates/satteri-arena/README.md)          | <a href="https://crates.io/crates/satteri-arena"><img alt="Satteri Arena Crates.io Version" src="https://img.shields.io/crates/v/satteri-arena"></a>                            |
| `satteri-ast`            | MDAST and HAST node types, codecs, tree operations, and conversion     | [README](./crates/satteri-ast/README.md)            | <a href="https://crates.io/crates/satteri-ast"><img alt="Satteri AST Crates.io Version" src="https://img.shields.io/crates/v/satteri-ast"></a>                                  |
| `satteri-plugin-api`     | Rust `Plugin` trait for Rust plugins, typed visitors, and runner       | [README](./crates/satteri-plugin-api/README.md)     | <a href="https://crates.io/crates/satteri-plugin-api"><img alt="Satteri Plugin API Crates.io Version" src="https://img.shields.io/crates/v/satteri-plugin-api"></a>             |
| `satteri-napi-binding`   | NAPI bindings exposing the Rust pipeline to JavaScript                 | [README](./crates/satteri-napi-binding/README.md)   | —                                                                                                                                                                               |
| `satteri-mdxjs-rs`       | MDX-to-JavaScript compiler, fork of [mdxjs-rs] adapted for OXC         | [README](./crates/satteri-mdxjs-rs/readme.md)       | <a href="https://crates.io/crates/satteri-mdxjs"><img alt="Satteri MDXJS Crates.io Version" src="https://img.shields.io/crates/v/satteri-mdxjs"></a>                            |
| `satteri-pulldown-cmark` | CommonMark parser with MDX extension support, fork of [pulldown-cmark] | [README](./crates/satteri-pulldown-cmark/README.md) | <a href="https://crates.io/crates/satteri-pulldown-cmark"><img alt="Satteri pulldown-cmark Crates.io Version" src="https://img.shields.io/crates/v/satteri-pulldown-cmark"></a> |

And the following npm package:

| Name             | Description                                          | README                                 | Registry                                                                                                                       |
| ---------------- | ---------------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| [`satteri`][npm] | TypeScript layer: plugin API and top-level functions | [README](./packages/satteri/README.md) | <a href="https://www.npmjs.com/package/satteri"><img alt="Satteri npm Version" src="https://img.shields.io/npm/v/satteri"></a> |

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
