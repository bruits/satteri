---
npm/satteri: patch
---

Fixed bundlers (esbuild/wrangler/Vite) failing to resolve `@napi-rs/wasm-runtime`
when `satteri` is a production dependency. The shipped WASI loaders
(`satteri_napi.wasi.cjs`, `satteri_napi.wasi-browser.js`) import `@napi-rs/wasm-runtime`
(which in turn pulls `@emnapi/core` and `@emnapi/runtime`), but those were declared
only as `devDependencies`, so they were never installed for consumers. They are now
`optionalDependencies`, so the WASM fallback path resolves while native-only users are
unaffected if installation is skipped.
