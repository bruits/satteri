# Sätteri

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/bruits/satteri?utm_source=badge)

High-performance Markdown and MDX processing. Parses and compiles in Rust, runs your plugins in JavaScript.

Check out the npm package's [documentation](./packages/satteri/README.md) for installation instructions, API reference, and usage examples, or [try it online on the playground](https://satteri.bruits.org/playground).

> Sätteri is an open-source project born from [Bruits](https://bruits.org/), a Rust-focused collective 💛

## Packages

Sätteri is a Rust + TypeScript monorepo containing the following Rust crates:

| Name                     | Description                                                            |
| ------------------------ | ---------------------------------------------------------------------- |
| `satteri`                | High-level Rust API for the pipeline: parse, convert, compile          |
| `satteri-arena`          | Arena allocator and binary buffer primitives                           |
| `satteri-ast`            | MDAST and HAST node types, codecs, tree operations, and conversion     |
| `satteri-plugin-api`     | Rust `Plugin` trait for Rust plugins, typed visitors, and runner       |
| `satteri-napi`           | NAPI bindings exposing the Rust pipeline to JavaScript                 |
| `satteri-mdxjs`          | MDX-to-JavaScript compiler, fork of [mdxjs-rs] adapted for OXC         |
| `satteri-pulldown-cmark` | CommonMark parser with MDX extension support, fork of [pulldown-cmark] |

And the following npm package:

| Name             | Description                                          | README                                 |
| ---------------- | ---------------------------------------------------- | -------------------------------------- |
| [`satteri`][npm] | TypeScript layer: plugin API and top-level functions | [README](./packages/satteri/README.md) |

## Contributing

Contributions are welcome! See our [Contributor Guide](https://github.com/bruits/satteri/blob/main/CONTRIBUTING.md) for help getting started.

## Acknowledgements

Sätteri builds on the work and knowledge of several open-source projects:

- [unifiedjs] -- the ecosystem of tools for processing content with syntax trees, including [remark](https://github.com/remarkjs/remark) and [rehype](https://github.com/rehypejs/rehype)
- [pulldown-cmark] -- CommonMark pull parser
- [mdxjs-rs] -- MDX compiler by Titus Wormer, forked to use pulldown-cmark and OXC

Special thanks to the following projects for paving the way for high-performance Rust <-> JavaScript interop:

- [oxc] -- Rust JavaScript parser and compiler by the OXC team, used for MDX compilation
- [Lightning CSS] -- Rust CSS parser with a optimized JavaScript Visitor API

[unifiedjs]: https://unifiedjs.com/
[pulldown-cmark]: https://github.com/pulldown-cmark/pulldown-cmark
[oxc]: https://oxc.rs
[Lightning CSS]: https://lightningcss.dev
[mdxjs-rs]: https://github.com/wooorm/mdxjs-rs
[npm]: https://www.npmjs.com/package/satteri

## License

Sätteri is licensed under the MIT License. See [LICENSE](https://github.com/bruits/satteri/blob/main/LICENSE) for details.
