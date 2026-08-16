---
cargo/satteri-wasm: minor
npm/@bruits/satteri-wasm: minor
npm/satteri: patch
---

Added a WASI-free `@bruits/satteri-wasm` package for browsers and single-threaded edge runtimes such as Cloudflare Workers. It exposes Markdown-to-HTML and materialized MDX-to-HAST bindings backed by the same Rust parser used by Sätteri's native package. A runtime-only build omits the JavaScript generator, and static WebAssembly exports use `.wasm` specifiers that Worker bundlers can recognize. The pure HAST reader is also available from `satteri/hast`.
