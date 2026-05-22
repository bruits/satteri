# Agents Guide

Sätteri is a Rust + TypeScript monorepo for high-performance Markdown/MDX processing, with an arena-allocated binary AST and a plugin system at both the MDAST and HAST levels.

## Useful Commands

```sh
cargo fmt --all                          # format Rust
cargo clippy --all --all-targets         # lint Rust
cargo test --all                         # test Rust
pnpm lint                                # oxlint + cargo clippy
pnpm format                              # oxfmt + cargo fmt
cd packages/satteri && pnpm test        # test JS (vitest)
cd packages/satteri && pnpm build       # build NAPI binding + TS
```

## Useful Resources

- In [CONTRIBUTING.md](./CONTRIBUTING.md): [Quality Guidelines](./CONTRIBUTING.md#quality-guidelines) applies to agents and humans equally, [Getting Started](./CONTRIBUTING.md#getting-started) documents each crate's role.
- The [README](./README.md) has the project overview, crate table, and pointer to the npm package README for installation and usage.
- `packages/satteri/src/index.ts` is the public API surface for the npm package.
- `crates/satteri-napi-binding/src/lib.rs` is the NAPI boundary — every function exposed to JS lives there.
- Spec tests for pulldown-cmark live under `crates/satteri-pulldown-cmark/specs/`.

## Agent Guardrails

- Do not create new documentation files to explain implementation.
- Do not add external dependencies without justification. Prefer the standard library and existing utilities.
- Match the current project structure, naming, and style; do not create parallel patterns and avoid duplication.
- All code, comments, documentation, commit messages, and user-facing output must be in English.
- The vendored `pulldown-cmark` intentionally diverges from upstream — do not "update" it to match upstream without explicit instruction.
- In general, mdast and hast are very similar, just with different node types and properties. As such, if an optimization or pattern is applied for one, it should also most likely also be applied to the other, unless there is a specific reason not to.
