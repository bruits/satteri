# @bruits/satteri-wasm

WASI-free WebAssembly bindings for Sätteri. The package uses the same Rust
parser and compiler as the native package, but only imports standard
WebAssembly host APIs, so it can run in browsers and single-threaded edge
runtimes such as Cloudflare Workers.

## Usage

Pass the `.wasm` module through your bundler's static WebAssembly import and
initialize it during module evaluation:

```ts
import wasmModule from "@bruits/satteri-wasm/satteri.wasm";
import { initSync, markdownToHtml } from "@bruits/satteri-wasm";

initSync({ module: wasmModule });

const html = markdownToHtml("# Hello");
```

`mdxToHast` returns a standard HAST root that preserves MDX JSX and expression
nodes without evaluating request data. `mdxToHastBuffer` exposes Sätteri's
compact wire format for low-level consumers. `mdxToJs` remains available for
build-time compilation; it should not be evaluated on an edge request path.

Runtime-only consumers should import `@bruits/satteri-wasm/runtime` and
`@bruits/satteri-wasm/runtime.wasm`. That build omits the JavaScript generator
while keeping Markdown, MDX, and HAST support.

The package does not use WASI, Node.js APIs, filesystem access, or worker
threads.
