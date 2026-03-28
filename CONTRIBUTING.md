# Contributing Guidelines

First, a huge **thank you** for dedicating your time to helping us improve Tryckeri ❤️

> [!Tip]
> **New to open source?** Check out [https://github.com/firstcontributions/first-contributions](https://github.com/firstcontributions/first-contributions) for helpful information on contributing

## Philosophy

Tryckeri aims to be a fast, correct, and extensible Markdown/MDX processing pipeline. We want to make it easy to get started, with minimal configuration, and sensible defaults. At the same time, we want to provide rich configuration options, and flexible workflows to cover more advanced use cases. Finally, Tryckeri should be easy to opt in and opt out, with little to none assumptions, conventions to follow, or lock-ins.

We're also committed to fostering a welcoming and respectful community. Any issue, PR, or discussion that violates our [code of conduct](./CODE_OF_CONDUCT.md) will be deleted, and the authors will be **banned**.

## Before Opening Issues

- **Do not report security vulnerabilities publicly** (e.g., in issues or discussions), please refer to our [security policy](./SECURITY.md).
- **Do not create issues for questions about using Tryckeri.** Instead, ask your question in our [GitHub Discussions](https://github.com/bruits/tryckeri/discussions/categories/q-a).
- **Do not create issues for ideas or suggestions.** Instead, share your thoughts in our [GitHub Discussions](https://github.com/bruits/tryckeri/discussions/categories/ideas).
- **Check for duplicates.** Look through existing issues and discussions to see if your topic has already been addressed.
- In general, provide as much detail as possible. No worries if it's not perfect, we'll figure it out together.

## Before Submitting Pull Requests (PRs)

- **Check for duplicates.** Look through existing PRs to see if your changes have already been submitted.
- **Check Clippy warnings.** Run `cargo clippy --all --all-targets` to ensure your code adheres to Rust's best practices.
- **Run formatting.** Run `cargo fmt --all` and `pnpm format` to ensure your code is properly formatted.
- **Write and run tests.** If you're adding new functionality or fixing a bug, please include tests to cover it. Run `cargo test --all` and `cd packages/tryckeri && pnpm test` to ensure all existing tests pass.
- Prefer small, focused PRs that address a single issue or feature. Larger PRs can be harder to review, and can often be broken down into smaller, more manageable pieces.
- PRs don't need to be perfect. Submit your best effort, and we will gladly assist in polishing the work.

## Quality Guidelines

- Prefer self-documenting code first, with expressive names and straightforward logic. Comments should explain _why_ (intent, invariants, trade-offs), not _how_, and not have separators/dividers/other visual noise. Variable and function names should be clear and descriptive, not cryptic abbreviations.
- Tests should assert observable behavior (inputs/outputs, effects), not internal implementation details. Keep tests deterministic and independent of global state.
- For Rust: use typed error enums (derived with `thiserror` where applicable). Prefer `?` propagation when possible, and reserve `.expect()`/`.unwrap()` for cases where failure is a programmer bug. Explicit `use` imports for standard library types (e.g. `use std::collections::HashMap;`).
- For TypeScript: strict types, no `any` shortcuts, and the same code style enforced by oxlint.
- We deeply value idiomatic, easy-to-maintain code. Avoid code duplication when possible. Prefer clarity over cleverness, and small focused functions over dark magic.

## Getting Started

Tryckeri is a Rust + TypeScript monorepo. The Rust workspace lives at the repository root (`Cargo.toml`), and the npm package lives under `packages/tryckeri`. The only prerequisites are the latest stable [Rust](https://www.rust-lang.org/) toolchain, [Node.js](https://nodejs.org/) (latest LTS), and [pnpm](https://pnpm.io/).

### Project Structure

#### tryckeri-mdast

`tryckeri-mdast` is the foundational data structure crate. It defines `MdastNode`, `MdastNodeType`, `StringRef` for zero-copy source references, `MdastArena` for owning all nodes, `MdastBuilder` for incremental tree construction, and the raw binary buffer format used to transfer trees between Rust and JavaScript with zero serialization overhead.

#### parser

`parser` bridges [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) events into `MdastArena`. It handles all supported extensions (tables, footnotes, strikethrough, task lists, math, heading attributes, YAML frontmatter, MDX) and produces the arena that flows through the rest of the pipeline.

#### tryckeri-hast

`tryckeri-hast` converts an MDAST arena into a HAST (HTML Abstract Syntax Tree), then serializes it to HTML. It also supports a binary HAST buffer format for efficient transfer to JavaScript.

#### mdxjs

`mdxjs` compiles MDX to JavaScript. This is a fork of [mdxjs-rs](https://github.com/wooorm/mdxjs-rs), adapted to use pulldown-cmark (instead of markdown-rs) and [OXC](https://oxc.rs/) (instead of SWC) for JavaScript AST manipulation and code generation. It supports static subtree optimization to collapse pure-HTML subtrees into raw strings.

#### tryckeri-plugin-api

`tryckeri-plugin-api` defines the Rust `Plugin` trait with typed visitor methods (`visit_heading`, `visit_link`, etc.), a `PluginRunner`, and a command/patch system for structural mutations. Plugins can inspect nodes, emit diagnostics, replace or remove nodes, and attach data.

#### pulldown-cmark / pulldown-cmark-escape

Vendored forks. `pulldown-cmark` adds MDX extension support (JSX tags, expressions, ESM imports/exports). When modifying these, be mindful that they diverge from upstream — check the spec test suites under `specs/`.

#### tryckeri-napi

NAPI bindings exposing `parse_to_buffer`, `parse_mdx_to_buffer`, `compile_mdx`, `parse_to_html`, and friends to Node.js. This is the bridge between the Rust pipeline and the JavaScript layer.

#### packages/tryckeri (npm)

The TypeScript layer provides `MdastReader` and `HastReader` (zero-copy binary buffer readers), `visitMdast`/`visitHast` (visitor pattern), `materializeNode`/`materializeTree` (lazy tree materialization), the plugin definition API (`defineMdastPlugin`, `defineHastPlugin`), a `Processor` for running plugins, and the top-level `compileMarkdownToHtml`/`compileMdxToJs` functions.

---

Thank you once again for contributing, we deeply appreciate all contributions, no matter how small or big.
