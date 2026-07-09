---
npm/satteri: patch
---

Fixes a crash in the browser and bundler builds where loading Sätteri could fail with `WebAssembly.Compile is disallowed on the main thread, if the buffer size is larger than 4KB`. The WebAssembly module now initializes asynchronously instead of compiling synchronously on the main thread.
