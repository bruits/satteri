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

Anything else (Linux arm64 or musl, browsers, edge runtimes) falls back to a WASI build.

## Browser usage

In a browser bundle, the WASI build replaces the native binding automatically. The WASI runtime needs `SharedArrayBuffer`, so the page must be cross-origin isolated using the following headers:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

See [napi-rs's server configuration guide](https://napi.rs/docs/concepts/webassembly#server-configuration) for more information.

## Using with Vite

`vite-plugin-satteri` lets you `import` `.md` and `.mdx` files directly in your Vite project. See [Usage with Vite](/docs/vite/) for more information.

## Versioning

Sätteri is pre-1.0, so expect breaking changes on minor version bumps. Every release is documented in the [CHANGELOG on GitHub](https://github.com/bruits/satteri/blob/main/packages/satteri/CHANGELOG.md).
