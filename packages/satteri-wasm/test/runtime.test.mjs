import { readFileSync } from "node:fs";
import test from "node:test";
import assert from "node:assert/strict";
import { initSync, markdownToHtml, mdxToHast, mdxToHastBuffer, mdxToJs } from "../index.js";
import { initSync as initRuntimeSync, mdxToHast as runtimeMdxToHast } from "../runtime.js";

function portableWasmModule(name) {
  const module = new WebAssembly.Module(readFileSync(new URL(`../dist/${name}`, import.meta.url)));
  const imports = WebAssembly.Module.imports(module);
  assert.equal(
    imports.some((entry) => /wasi|node|thread/i.test(`${entry.module}:${entry.name}`)),
    false,
  );
  return module;
}

const wasmModule = portableWasmModule("satteri_wasm_bg.wasm");
initSync({ module: wasmModule });

const runtimeWasmModule = portableWasmModule("satteri_runtime_bg.wasm");
initRuntimeSync({ module: runtimeWasmModule });

test("runs the Rust pipeline through the package wrapper", () => {
  assert.equal(
    markdownToHtml("# edge\n\nHello **world**."),
    "<h1>edge</h1>\n<p>Hello <strong>world</strong>.</p>\n",
  );

  const hast = mdxToHast('# edge\n\n<Component label="Go" />');
  assert.equal(hast.type, "root");
  assert.equal(hast.children[0].type, "element");
  assert.equal(hast.children[0].tagName, "h1");
  const component = hast.children.find((child) => child.type === "mdxJsxFlowElement");
  assert.ok(component);
  assert.equal(component.name, "Component");
  assert.deepEqual(component.attributes, [{ type: "mdxJsxAttribute", name: "label", value: "Go" }]);

  const buffer = mdxToHastBuffer("# edge");
  assert.ok(buffer instanceof Uint8Array);
  assert.ok(buffer.length > 32);

  assert.ok(mdxToJs("# edge").length > 0);
});

test("runtime build materializes MDX HAST", () => {
  const hast = runtimeMdxToHast('# edge\n\n<Component label="Go" />');
  assert.equal(hast.type, "root");
  assert.equal(hast.children[0].type, "element");
  assert.equal(hast.children[0].tagName, "h1");
  const component = hast.children.find((child) => child.type === "mdxJsxFlowElement");
  assert.ok(component);
  assert.equal(component.name, "Component");
  assert.throws(() => runtimeMdxToHast("<Component"), /MDX parse error/);
});
