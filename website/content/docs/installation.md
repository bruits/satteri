---
title: "Installation"
description: "Add Sätteri to a JavaScript project."
section: "getting-started"
order: 10
---

Sätteri ships as a regular npm package. The Rust core is precompiled to native binaries via napi-rs, so you don't need a toolchain to use it.

## Install

{{ install pkg="satteri" /}}

## Supported runtimes

Sätteri ships native binaries for:

- macOS (Apple Silicon and Intel)
- Linux (x86_64, glibc)
- Windows (x86_64)

Anything else, such as browsers and edge runtimes, uses the [WebAssembly build](#webassembly-build).

## WebAssembly build

Browsers, Cloudflare Workers, and platforms without a native binary run Sätteri as WebAssembly. That build ships as a separate package to keep native installs small, so install it next to `satteri` and keep both on the same version:

{{ install pkg="satteri @bruits/satteri-wasm32-wasip1" /}}

Then import `satteri` as usual. In a Worker, Wrangler picks the WebAssembly build on its own, so you don't need the `nodejs_compat` flag or any setup call.

## Using with Vite

`vite-plugin-satteri` lets you `import` `.md` and `.mdx` files directly in your Vite project. See [Usage with Vite](/docs/vite/) for more information.

## Versioning

Sätteri is pre-1.0, so expect breaking changes on minor version bumps. Every release is documented in the [CHANGELOG on GitHub](https://github.com/bruits/satteri/blob/main/packages/satteri/CHANGELOG.md).
