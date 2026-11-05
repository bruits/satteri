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

## Browsers and Cloudflare Workers

In a browser bundle or a Cloudflare Worker, the WASI build replaces the native binding. It ships as a separate optional package, `@bruits/satteri-wasm32-wasip1`, marked `cpu: wasm32` so native installs stay lean. Package managers skip mismatched architectures by default, so you have to opt in.

With pnpm, add the `wasm32` architecture in `pnpm-workspace.yaml`:

```yaml
supportedArchitectures:
  cpu:
    - current
    - wasm32
```

Yarn exposes the same `supportedArchitectures` setting in `.yarnrc.yml`. For other package managers, see [napi-rs's WebAssembly guide](https://napi.rs/docs/concepts/webassembly).

In a Worker, import `satteri` as usual. Wrangler picks the WASI build through the `workerd` export condition, so no `nodejs_compat` flag or manual initialization is needed.

## Using with Vite

`vite-plugin-satteri` lets you `import` `.md` and `.mdx` files directly in your Vite project. See [Usage with Vite](/docs/vite/) for more information.

## Versioning

Sätteri is pre-1.0, so expect breaking changes on minor version bumps. Every release is documented in the [CHANGELOG on GitHub](https://github.com/bruits/satteri/blob/main/packages/satteri/CHANGELOG.md).
