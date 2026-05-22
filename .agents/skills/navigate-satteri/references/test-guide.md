# Satteri Test Guide

Complete reference for testing: where tests live, how to add them, how to run them.

## Test Locations Overview

### Rust Tests

| Location | What it tests |
|----------|--------------|
| `crates/satteri-pulldown-cmark/specs/*.txt` | Spec test cases (CommonMark extensions) |
| `crates/satteri-pulldown-cmark/third_party/*.txt` | CommonMark and GFM upstream specs |
| `crates/satteri-pulldown-cmark/tests/` | Integration tests (HTML output, MDX, errors) |
| `crates/satteri-ast/tests/` | Arena construction, HTML rendering, codec roundtrips, rebuild, positions |
| `crates/satteri-plugin-api/tests/` | Plugin basics, commands, data, runner, rebuild integration |
| `crates/satteri-mdxjs-rs/tests/test.rs` | MDX-to-JS compilation end-to-end |
| Inline `#[cfg(test)]` modules | Unit tests in ~20 files across all crates |

### JavaScript Tests

| Location | What it tests |
|----------|--------------|
| `packages/satteri/test/compile.test.ts` | Top-level API: `markdownToHtml`, `mdxToJs`, frontmatter, plugin integration |
| `packages/satteri/test/visitor.test.ts` | MDAST visitor handle API: subscriptions, callbacks, mutations |
| `packages/satteri/test/hast-visitor.test.ts` | HAST visitor handle API: element/text callbacks, mutations |
| `packages/satteri/test/html-plugin.test.ts` | End-to-end MDAST/HAST plugin pipeline affecting HTML output |
| `packages/satteri/test/plugin.test.ts` | `defineMdastPlugin` identity and validation |
| `packages/satteri/test/materializer.test.ts` | MDAST tree materialization from binary arena buffers |
| `packages/satteri/test/mdast-reader.test.ts` | Low-level `MdastReader` binary buffer reading |
| `packages/satteri/test/conformance/` | Conformance against remark/rehype ecosystem (19 test files) |
| `packages/satteri/test/fixtures.ts` | Test utility: builds binary arena buffers in pure JS |

### Key Conformance Test Files

| File | Reference implementation |
|------|------------------------|
| `conformance/mdast.test.ts` | `remark-parse` + `remark-gfm` |
| `conformance/hast.test.ts` | `remark-parse` + `remark-gfm` + `remark-rehype` |
| `conformance/mdx.test.ts` | `@mdx-js/mdx` |
| `conformance/mdx-ast.test.ts` | `remark` + `remark-mdx` |
| `conformance/math.test.ts` | `remark-math` |
| `conformance/frontmatter.test.ts` | `remark-frontmatter` |
| `conformance/directive.test.ts` | `remark-directive` |
| `conformance/fuzz.test.ts` | Property-based fuzz tests with `fast-check` |
| `conformance/spec-deltas.test.ts` | Known deviations from CommonMark spec |

### Inline Unit Test Locations

Major `#[cfg(test)]` modules in the Rust codebase:

| File | Line | What it tests |
|------|------|--------------|
| `satteri-arena/src/arena.rs` | ~236 | Allocation, position roundtrip, children/parent, string resolution |
| `satteri-arena/src/builder.rs` | ~302 | Open/close, auto-close on finish, leaf creation |
| `satteri-arena/src/node.rs` | ~87 | Size assertions (52-byte node, 8-byte StringRef) |
| `satteri-arena/src/line_index.rs` | ~70 | Byte offset to line/column mapping |
| `satteri-ast/src/mdast/codec.rs` | ~608 | MDAST type_data encode/decode roundtrips |
| `satteri-ast/src/hast/codec.rs` | ~62 | HAST type_data encode/decode roundtrips |
| `satteri-ast/src/hast/properties.rs` | ~467 | Property to attribute name mapping |
| `satteri-ast/src/convert.rs` | ~1999 | MDAST-to-HAST conversion |
| `satteri-ast/src/rebuild.rs` | ~609 | All patch types, composition, error cases |
| `satteri-ast/src/walk.rs` | ~510 | Subscription matching, binary serialization |
| `satteri-ast/src/text_content.rs` | ~52 | Text extraction |
| `satteri-plugin-api/src/js_commands.rs` | ~1172 | Command buffer parsing, set-property, replace, directives |
| `satteri-pulldown-cmark/src/parse.rs` | ~2728 | Core parser internals |
| `satteri-pulldown-cmark/src/firstpass.rs` | ~3851 | Block-level first pass |
| `satteri-pulldown-cmark/src/scanners.rs` | ~1596 | Inline scanning patterns |

## Spec Test Format

Spec tests live in `.txt` files under `crates/satteri-pulldown-cmark/specs/` (custom extensions) and `crates/satteri-pulldown-cmark/third_party/` (CommonMark/GFM upstream).

### Format of a spec test case

```
Optional prose description of what this tests.

```````````````````````````````` example
markdown input here
can be multiple lines
.
expected HTML output here
can also be multiple lines
````````````````````````````````
```

Rules:
- Opening fence: exactly 32 backticks followed by ` example`
- Input and expected output are separated by a line containing only `.`
- Closing fence: exactly 32 backticks
- Tabs in specs are represented as the arrow character and converted to `\t` at parse time
- Disable a test by using `DISABLED example` instead of `example`

### Extension-specific flags

Append to the `example` keyword to enable specific parser options:

| Suffix | Parser option enabled |
|--------|---------------------|
| `example_smartpunct` | Smart punctuation |
| `example_metadata_blocks` | YAML/TOML metadata blocks |
| `example_super_sub` | Superscript/subscript |
| `example_wikilinks` | Wikilinks |
| `example_deflists` | Definition lists |
| `example_container_extensions` | Directives |

Each spec file is mapped to a base set of parser options in the `base_options_for_spec()` function in `build.rs`. Tests within a file inherit that base, and the flag suffix adds on top.

### Adding a new spec test

1. Open the appropriate `.txt` file under `specs/` (or create one for a new extension)
2. Append the test case in the format above
3. Run `cargo test -p satteri-pulldown-cmark` -- the `build.rs` script auto-generates Rust test files from the spec files

### Spec files inventory

| File | What it covers |
|------|---------------|
| `specs/table.txt` | GFM table extension |
| `specs/footnotes.txt` | Footnote definitions and references |
| `specs/strikethrough.txt` | GFM strikethrough |
| `specs/math.txt` | Math blocks and inline math |
| `specs/heading_attrs.txt` | Heading attribute syntax |
| `specs/metadata_blocks.txt` | YAML/TOML frontmatter |
| `specs/regression.txt` | Regression test cases |
| `specs/super_sub.txt` | Superscript/subscript |
| `specs/wikilinks.txt` | Wikilink syntax |
| `specs/container_extensions.txt` | Directive syntax |

## Conformance Test Pattern

JS conformance tests compare Satteri output against the canonical remark/rehype ecosystem. The pattern is defined in `test/conformance/helpers.ts`.

### MDAST conformance

```ts
assertMdastConformance("# Hello");
// reference = unified().use(remarkParse).use(remarkGfm).parse(md)
// actual    = markdownToMdast(md, { features: { frontmatter: false, math: false } })
// Deeply compared after JSON serialization
```

### HAST conformance

```ts
assertHastConformance("# Hello");
// reference = remarkParse -> remarkGfm -> remarkRehype -> runSync
// actual    = markdownToHast(md, { features: { frontmatter: false, math: false } })
// Compared with normalizations (align->style, strip data)
```

### Extension conformance

```ts
assertExtMdastConformance("$x^2$", ["math"]);
// Builds a custom reference processor with the specified remark plugins
// Enables matching features on the Satteri side
```

### MDX conformance

```ts
await assertMdxConformance("<Foo bar={1}/>", { Foo });
// Both @mdx-js/mdx and Satteri evaluate the MDX
// Rendered to HTML via React's renderToStaticMarkup
// Normalized HTML strings compared
```

### Normalizations applied during comparison

- `data` properties are stripped (remark attaches internal data)
- Table `align` properties are converted to `style="text-align: ..."` (different HAST conventions)
- HTML void elements normalized (`<br>` / `<br/>` to `<br />`)
- HTML entities normalized
- Position columns may differ for non-ASCII inputs (byte vs. codepoint counting), use `assertExtMdastConformanceNoPosition` in those cases

## Running Tests

### Rust

```sh
# All Rust tests
cargo test --all

# Specific crate
cargo test -p satteri-pulldown-cmark
cargo test -p satteri-ast
cargo test -p satteri-plugin-api
cargo test -p satteri-mdxjs-rs

# Specific test by name substring
cargo test -p satteri-pulldown-cmark regression_test_1

# Specific integration test file
cargo test -p satteri-pulldown-cmark --test html
cargo test -p satteri-pulldown-cmark --test mdx
cargo test -p satteri-pulldown-cmark --test errors

# Spec suite subsets
cargo test -p satteri-pulldown-cmark suite::table
cargo test -p satteri-pulldown-cmark suite::regression
cargo test -p satteri-pulldown-cmark suite::spec
```

### JavaScript

```sh
# All JS tests (must be run from packages/satteri/)
pnpm test
# This runs: vitest run

# Specific test file
pnpm vitest run test/compile.test.ts
pnpm vitest run test/conformance/mdast.test.ts
pnpm vitest run test/conformance/fuzz.test.ts

# Filter by test name
pnpm vitest run -t "heading"
```

**Important:** If native Rust code changed, rebuild before running JS tests:
```sh
pnpm build
# This runs: napi build + tsc
```

### Linting and formatting (required before PRs)

```sh
cargo clippy --all --all-targets    # Rust lint
cargo fmt --all                      # Rust format
pnpm lint                            # oxlint + cargo clippy + knip
pnpm format                          # oxfmt + cargo fmt
```

### Full verification sequence

After any change, the minimum verification before submitting:
```sh
cargo clippy --all --all-targets
cargo fmt --all --check
cargo test --all
cd packages/satteri && pnpm build && pnpm test
```

## Which Tests to Run Based on What Changed

| Changed area | Tests to run |
|-------------|-------------|
| `crates/satteri-pulldown-cmark/` | `cargo test -p satteri-pulldown-cmark` + conformance tests |
| `crates/satteri-ast/src/mdast/` | `cargo test -p satteri-ast` + JS mdast conformance |
| `crates/satteri-ast/src/hast/` | `cargo test -p satteri-ast` + JS hast conformance + HTML tests |
| `crates/satteri-ast/src/convert.rs` | `cargo test -p satteri-ast` + JS hast conformance |
| `crates/satteri-ast/src/rebuild.rs` | `cargo test -p satteri-ast` + JS plugin tests |
| `crates/satteri-ast/src/walk.rs` | `cargo test -p satteri-ast` + JS visitor tests |
| `crates/satteri-plugin-api/` | `cargo test -p satteri-plugin-api` + JS plugin/visitor tests |
| `crates/satteri-plugin-api/src/js_commands.rs` | `cargo test -p satteri-plugin-api` + all JS tests |
| `crates/satteri-mdxjs-rs/` | `cargo test -p satteri-mdxjs-rs` + JS MDX conformance |
| `crates/satteri-napi-binding/` | JS tests (no Rust tests in this crate) |
| `packages/satteri/src/mdast/` | JS mdast/visitor/plugin tests |
| `packages/satteri/src/hast/` | JS hast-visitor/html-plugin tests |
| `packages/satteri/src/compile.ts` | `pnpm test` (all JS tests) |
| `packages/satteri/src/command-buffer.ts` | JS plugin/visitor tests + Rust js_commands tests |

When in doubt, run the full verification sequence.
