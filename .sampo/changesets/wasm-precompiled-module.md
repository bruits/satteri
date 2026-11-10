---
npm/satteri: patch
---

Added `globalThis.__SATTERI_WASM_MODULE__`: when it holds a compiled `WebAssembly.Module` (or the wasm's bytes, or a promise of either) before `satteri` is imported, the browser WASI build uses it instead of fetching and compiling the wasm again.
