---
title: "Installation"
description: "Add Sätteri to a JavaScript project."
section: "getting-started"
order: 10
---

Sätteri ships as a regular npm package. The Rust core is precompiled to
native binaries via napi-rs, so you don't need a toolchain to use it.

## Install

```bash
pnpm add satteri
```

```bash
npm install satteri
```

```bash
yarn add satteri
```

## Supported runtimes

Sätteri ships native binaries for:

- macOS (Apple Silicon and Intel)
- Linux (x86_64, glibc)
- Windows (x86_64)

Anything else (Linux arm64 or musl, browsers, edge runtimes) falls back to a
WASI build.

## Browser usage

In a browser bundle, the WASI build replaces the native binding
automatically. Two caveats:

- The WASI runtime needs `SharedArrayBuffer`, which requires the page to
  be cross-origin isolated. Serve it with `Cross-Origin-Opener-Policy:
same-origin` and `Cross-Origin-Embedder-Policy: require-corp`.
- The WASM file is a few MB; serve it with gzip or brotli compression and
  a long `Cache-Control` so the first compile isn't blocked on the
  download.

The WASI binding initializes lazily on first import. Defer that import
(via `import()` inside an event handler or `requestIdleCallback`) so the
WASM download doesn't fight initial page paint.

## Versioning

Sätteri is pre-1.0, so expect breaking changes on minor version bumps. Every
release is documented in the [CHANGELOG on
GitHub](https://github.com/bruits/satteri/blob/main/packages/satteri/CHANGELOG.md).
