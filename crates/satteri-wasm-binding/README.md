# satteri-wasm

WASI-free WebAssembly bindings for Sätteri. The crate exposes the Rust
Markdown and MDX pipeline through browser and single-threaded edge-compatible
WebAssembly host APIs.

The generated JavaScript package is documented in
[`packages/satteri-wasm`](../../packages/satteri-wasm/README.md).

## Development

Build the generated package with:

```sh
wasm-pack build crates/satteri-wasm-binding --target web --release \
  --out-dir ../../packages/satteri-wasm/dist --out-name satteri_wasm --no-pack
```

Refer to [CONTRIBUTING.md](../../CONTRIBUTING.md) for development setup and
workflow details.
