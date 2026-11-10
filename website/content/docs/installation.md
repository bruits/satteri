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

In a browser bundle, the WASI build replaces the native binding. It ships as a separate optional package, `@bruits/satteri-wasm32-wasi`, marked `cpu: wasm32` so native installs stay lean. Package managers skip mismatched architectures by default, so you have to opt in.

With pnpm, add the `wasm32` architecture in `pnpm-workspace.yaml`:

```yaml
supportedArchitectures:
  cpu:
    - current
    - wasm32
```

Yarn exposes the same `supportedArchitectures` setting in `.yarnrc.yml`. For other package managers, see [napi-rs's WebAssembly guide](https://napi.rs/docs/concepts/webassembly).

The WASI runtime also needs `SharedArrayBuffer`, so the page must be cross-origin isolated, using the following headers:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

See [napi-rs's server configuration guide](https://napi.rs/docs/concepts/webassembly#server-configuration) for more information.

### Reusing a compiled module

When `satteri` is first imported in a browser, the WASI build fetches and compiles `satteri_napi.wasm32-wasi.wasm`. To compile it once and share it, for example across Web Workers, set `globalThis.__SATTERI_WASM_MODULE__` to a `WebAssembly.Module` (or the file's bytes, or a promise of either) before importing `satteri`:

```ts
// Page: compile once, then hand the module to each worker.
// wasmUrl is the URL of @bruits/satteri-wasm32-wasi/satteri_napi.wasm32-wasi.wasm
const module = await WebAssembly.compileStreaming(fetch(wasmUrl));
worker.postMessage(module);

// Worker
self.onmessage = async (event) => {
  Object.assign(globalThis, { __SATTERI_WASM_MODULE__: event.data });
  const { markdownToHtml } = await import("satteri");
  // ...
};
```

Use a dynamic `import()` here: a static `import` runs before the code that sets the global.

## Using with Vite

`vite-plugin-satteri` lets you `import` `.md` and `.mdx` files directly in your Vite project. See [Usage with Vite](/docs/vite/) for more information.

## Versioning

Sätteri is pre-1.0, so expect breaking changes on minor version bumps. Every release is documented in the [CHANGELOG on GitHub](https://github.com/bruits/satteri/blob/main/packages/satteri/CHANGELOG.md).
