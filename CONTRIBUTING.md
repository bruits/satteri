# Contributing Guidelines

Thanks for wanting to contribute to Sätteri!

> [!Tip]
> **New to open source?** Check out [https://github.com/firstcontributions/first-contributions](https://github.com/firstcontributions/first-contributions) for helpful information on contributing

## Philosophy

Sätteri is a fast, correct, and extensible Markdown/MDX processing pipeline. Performance is the core focus, but not at the cost of correctness or maintainability.

Any issue, PR, or discussion that violates our [code of conduct](./CODE_OF_CONDUCT.md) will be deleted and the authors banned.

## Before Opening Issues

- Questions about using Sätteri belong in [GitHub Discussions (Q&A)](https://github.com/bruits/satteri/discussions/categories/q-a), not issues.
- Ideas and suggestions belong in [GitHub Discussions (Ideas)](https://github.com/bruits/satteri/discussions/categories/ideas), not issues.
- Check for duplicates before opening anything.
- Provide as much detail as you can. It doesn't need to be perfect.

## Before Submitting Pull Requests

- Check for duplicate PRs.
- Run `cargo clippy --all --all-targets` and fix any warnings.
- Run `cargo fmt --all` and `pnpm format`.
- If you're adding functionality or fixing a bug, include tests. Run `cargo test --all` and `cd packages/satteri && pnpm test`.
- Prefer small, focused PRs. Large ones can often be split up.
- PRs don't need to be perfect. Submit your best effort and we'll help polish it.

## Quality Guidelines

- Self-documenting code first. Expressive names, straightforward logic. Comments should explain _why_, not _how_. No separator comments or visual noise.
- Tests should assert observable behavior (inputs/outputs), not implementation details. Keep them deterministic.
- Rust: typed error enums (with `thiserror` where applicable), `?` propagation, `.expect()`/`.unwrap()` only for programmer bugs. Explicit `use` imports for standard library types.
- TypeScript: strict types, no `any`, code style enforced by oxlint.
- Clarity over cleverness. Small focused functions. Avoid duplication.

## Getting Started

Sätteri is a Rust + TypeScript monorepo. The Rust workspace lives at the repository root (`Cargo.toml`), and the npm package lives under `packages/satteri`.

### Prerequisites

- latest stable [Rust](https://www.rust-lang.org/)
- [Node.js](https://nodejs.org/) (latest LTS)
- [pnpm](https://pnpm.io/)

### Initial set-up

- Install Node dependencies by running `pnpm install`
- Build the project by running `cargo build`

### Project Structure

#### `satteri-arena`

Arena allocator and binary buffer primitives shared across crates.

#### `satteri-ast`

MDAST and HAST node types, codecs, tree operations, and mdast-to-hast conversion. Also defines the binary buffer formats used to transfer trees between Rust and JavaScript.

#### `satteri-pulldown-cmark`

A fork of [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark), adapted for Sätteri. Bridges pulldown-cmark events into the `satteri-ast` arena. Handles all supported extensions (tables, footnotes, strikethrough, task lists, math, heading attributes, YAML frontmatter, MDX). When modifying, be mindful that while it diverges from upstream, it's still supposed to be CommonMark compliant. Check the spec test suites under `specs/`.

#### `satteri-mdxjs-rs`

MDX-to-JavaScript compiler. A fork of [mdxjs-rs](https://github.com/wooorm/mdxjs-rs), adapted to use pulldown-cmark (instead of markdown-rs) and [OXC](https://oxc.rs/) (instead of SWC) for JavaScript AST manipulation and code generation. There's very little code remaining in it from the original mdxjs-rs, but it still serves as a useful reference for how to compile MDX to JavaScript.

#### `satteri-plugin-api`

Rust `Plugin` trait with typed visitor methods, a `PluginRunner`, and a command/patch system for structural mutations.

#### `satteri-napi-binding`

NAPI bindings exposing parsing, compilation, handle management, and tree walking to JavaScript.

#### `satteri` (Rust crate)

High-level Rust API tying the pipeline together: parse, convert, compile.

#### `packages/satteri` (npm)

The TypeScript layer. Provides the public functions (`markdownToHtml`, `mdxToJs`, etc), and the plugin definition API (`defineMdastPlugin`, `defineHastPlugin`).
