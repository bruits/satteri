# Tryckeri

High-performance Markdown and MDX processing. Powered by a Rust arena backend with zero-copy binary transfer to JavaScript, and a plugin system for custom transformations.

Don't know where to start? Check out our npm package's [ documentation](./packages/tryckeri/README.md).

## Packages

Tryckeri is a Rust + TypeScript monorepo, that contains the following crates (Rust packages):

| Name                    | Description                                                                            | Registry | README                                             |
| ----------------------- | -------------------------------------------------------------------------------------- | -------- | -------------------------------------------------- |
| `tryckeri-mdast`        | Arena-allocated MDAST with zero-copy string references and binary buffer format        | *WIP*    | [README](./crates/mdast/README.md)                 |
| `parser`                | Bridges pulldown-cmark events into an `MdastArena` (tables, footnotes, math, MDX, …)   | *WIP*    | [README](./crates/parser/README.md)                |
| `tryckeri-hast`         | MDAST → HAST conversion and HTML serialization, with binary HAST buffer support        | *WIP*    | [README](./crates/hast/README.md)                  |
| `tryckeri-plugin-api`   | Rust `Plugin` trait, typed visitors, `PluginRunner`, and command/patch mutation system | *WIP*    | [README](./crates/plugin-api/README.md)            |
| `tryckeri-napi`         | NAPI bindings exposing the Rust pipeline to Node.js                                    | *WIP*    | [README](./crates/napi-binding/README.md)          |
| `mdxjs`                 | MDX → JavaScript compiler — fork of [mdxjs-rs] adapted for pulldown-cmark and [OXC]    | *WIP*    | [README](./crates/mdxjs-rs/readme.md)              |
| `pulldown-cmark`        | Vendored CommonMark parser with MDX extension support                                  | *WIP*    | *—*                                                |
| `pulldown-cmark-escape` | Vendored HTML escape utilities from the pulldown-cmark project                         | *WIP*    | [README](./crates/pulldown-cmark-escape/README.md) |
| `tryckeri-bench`        | Benchmarks and profiling harnesses for the pipeline                                    | *WIP*    | [README](./crates/bench/README.md)                 |

Tryckeri also includes the following npm package:

| Name              | Description                                                                                           | Registry | README                                  |
| ----------------- | ----------------------------------------------------------------------------------------------------- | -------- | --------------------------------------- |
| [`tryckeri`][npm] | TypeScript layer: binary buffer readers, visitor pattern, plugin API, and top-level compile functions | _WIP_    | [README](./packages/tryckeri/README.md) |

## Acknowledgements

Tryckeri builds on the work of several open-source projects:

- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) — CommonMark pull parser, vendored here with MDX extensions.
- [mdxjs-rs] — original MDX compiler by Titus Wormer, forked to use pulldown-cmark and OXC.
- [OXC] — fast JavaScript/TypeScript toolchain used for AST manipulation and code generation.
- [NAPI-RS] — Rust ↔ Node.js bridge that makes zero-copy binary transfer possible.

Tryckeri is an open-source project born from [Bruits](https://bruits.org/), a Rust-focused collective 💛

[mdxjs-rs]: https://github.com/wooorm/mdxjs-rs
[OXC]: https://oxc.rs/
[NAPI-RS]: https://napi.rs/
[npm]: https://www.npmjs.com/package/tryckeri
