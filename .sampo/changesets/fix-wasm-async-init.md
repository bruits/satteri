---
npm/satteri: patch
---

Fixes a crash in the browser and bundler builds where loading Satteri could fail with `WebAssembly.Compile is disallowed on the main thread, if the buffer size is larger than 4KB`. The WebAssembly module now initializes asynchronously instead of compiling synchronously on the main thread, so it loads consistently across browsers and versions. Previously this could succeed on one machine but fail on another depending on the browser's main-thread compilation limit.
