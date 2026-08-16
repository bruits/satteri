import { readFileSync } from "node:fs";
import test from "node:test";
import assert from "node:assert/strict";
import {
  initSync,
  markdownToHtml,
  mdxToHast,
  mdxToJs,
} from "../index.js";

const wasmModule = new WebAssembly.Module(
  readFileSync(new URL("../dist/satteri_wasm_bg.wasm", import.meta.url)),
);
const imports = WebAssembly.Module.imports(wasmModule);
assert.equal(
  imports.some(({ module, name }) => /wasi|node|thread/i.test(`${module}:${name}`)),
  false,
);
initSync({ module: wasmModule });

test("runs the Rust pipeline through the package wrapper", () => {
  assert.equal(
    markdownToHtml("# edge\n\nHello **world**."),
    "<h1>edge</h1>\n<p>Hello <strong>world</strong>.</p>\n",
  );

  const hast = mdxToHast("# edge\n\n<Component />");
  assert.ok(hast instanceof Uint8Array);
  assert.ok(hast.length > 32);

  assert.ok(mdxToJs("# edge").length > 0);
});
