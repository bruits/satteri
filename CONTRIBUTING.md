# Contributing Guidelines

First, a huge **thank you** for dedicating your time to helping us improve Sätteri ❤️

> [!Tip]
> **New to open source?** Check out [https://github.com/firstcontributions/first-contributions](https://github.com/firstcontributions/first-contributions) for helpful information on contributing

## Philosophy

Sätteri is a fast, correct, and extensible Markdown/MDX processing pipeline. Performance is the core focus, but not at the cost of correctness or maintainability.

We're also committed to fostering a welcoming and respectful community. Any issue, PR, or discussion that violates our [code of conduct](./CODE_OF_CONDUCT.md) will be deleted, and the authors will be **banned**.

## Before Opening Issues

- **Do not report security vulnerabilities publicly** (e.g., in issues), please refer to our [security policy](./SECURITY.md).
- **Do not create issues for questions about using Sätteri.** Instead, ask on our [Discord](https://discord.com/invite/84pd4QtmzA).
- **For ideas or feature suggestions**, open a [feature request issue](https://github.com/bruits/satteri/issues/new?template=02-feature-request.yml) or chat about it first on [Discord](https://discord.com/invite/84pd4QtmzA).
- **Check for duplicates.** Look through existing issues to see if your topic has already been addressed.
- In general, provide as much detail as possible. No worries if it's not perfect, we'll figure it out together.

## Before submitting Pull Requests (PRs)

- **Check for duplicates.** Look through existing PRs to see if your changes have already been submitted.
- **Lint.** Run `pnpm lint` to lint both the Rust (`cargo clippy`) and TypeScript (`oxlint`) code.
- **Format.** Run `pnpm format` to format both the Rust (`cargo fmt`) and TypeScript (`oxfmt`) code.
- **Write and run tests.** If you're adding new functionality or fixing a bug, please include tests to cover it. Run `cargo test --all` and `cd packages/satteri && pnpm test` to ensure all existing tests pass.
- **Write a changeset.** Run `sampo add` to create a new changeset file describing your changes.
- Prefer small, focused PRs that address a single issue or feature. Larger PRs can be harder to review, and can often be broken down into smaller, more manageable pieces.
- PRs don't need to be perfect. Submit your best effort, and we will gladly assist in polishing the work.

## Quality Guidelines

- Prefer self-documenting code first, with expressive names and straightforward logic. Comments should explain _why_ (intent, invariants, trade-offs), not _how_. Variable and function names should be clear and descriptive, not cryptic abbreviations. Avoid hidden state and side effects.
- Tests should assert observable behavior (inputs/outputs, effects), not internal implementation details. Keep tests deterministic and independent of global state.
- For errors, use typed error enums in library crates (derived with `thiserror`). Per-crate `pub type Result<T>` aliases for ergonomic signatures. Add context at the boundary (NAPI binding) rather than deep in core, keep library error messages concise.
- Prefer `?` propagation when possible, and reserve `.expect()`/`.unwrap()` for cases where failure is a programmer bug (e.g. hardcoded regex literals, test helpers).
- TypeScript: strict types, no `any`, code style enforced by oxlint.
- Document any new public APIs, configuration options, or user-facing changes in the relevant README files. If you're unsure where or how to document something, just ask and we'll help you out.
- We deeply value idiomatic, easy-to-maintain Rust code. Avoid code duplication when possible. And prefer clarity over cleverness, and small focused functions over dark magic.
- Explicit `use` imports for standard library types (e.g. `use std::collections::HashMap;`).

## Writing Changesets

Sätteri uses [Sampo](https://github.com/bruits/sampo) to manage changelogs and versioning. Every user-facing change should ship with a changeset that lands in the changelog of the next release. Run `sampo add` to create one.

**Structure:**

1. **Verb:** `Added`, `Removed`, `Fixed`, `Changed`, `Deprecated`, or `Improved`.
2. **Description**.
3. **Usage example (optional):** A minimal snippet if it clarifies the change.

**Description guidelines:** concise (1-2 sentences), specific (mention the command/option/API), actionable (what changed, not why), user-facing (written for changelog readers), and in English. Don't detail internal implementation changes.

## Getting Started

Sätteri is a Rust + TypeScript monorepo. The Rust workspace lives at the repository root (`Cargo.toml`), and the npm packages live under `packages/`.

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

#### `packages/satteri-expressive-code` (npm)

HAST plugin rendering code blocks with [Expressive Code](https://expressive-code.com) (Shiki highlighting, frames, copy button); the Sätteri equivalent of `rehype-expressive-code`.

#### `packages/vite-plugin-satteri` (npm)

Vite plugin that imports `.md` (rendered HTML) and `.mdx` (JSX component) files through Sätteri.

---

Thank you once again for contributing, we deeply appreciate all contributions, no matter how small or big.
