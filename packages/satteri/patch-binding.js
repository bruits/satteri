import { readFileSync, writeFileSync } from "node:fs";

// napi-rs regenerates these files on every build, so the changes are re-applied after each one.
function patchFile(fileName, isPatched, patch) {
  const path = new URL(`./${fileName}`, import.meta.url);
  const content = readFileSync(path, "utf-8");
  if (isPatched(content)) return;
  const patched = patch(content);
  if (patched === content) {
    throw new Error(`patch-binding: the generated ${fileName} no longer matches the patch`);
  }
  writeFileSync(path, patched);
}

patchFile(
  "index.js",
  (content) => content.includes("webcontainer-fallback"),
  (content) =>
    content.replace(
      "\nif (!nativeBinding) {",
      (s) =>
        `
if (!nativeBinding && globalThis.process?.versions?.['webcontainer']) {
  try {
    nativeBinding = require('./webcontainer-fallback.cjs');
  } catch (err) {
    loadErrors.push(err)
  }
}
` + s,
    ),
);

// Lets a page compile the wasm once and hand it to each worker, instead of every worker fetching
// and compiling it again. The value is passed straight to `instantiateNapiModule`.
patchFile(
  "satteri_napi.wasi-browser.js",
  (content) => content.includes("__SATTERI_WASM_MODULE__"),
  (content) =>
    content.replace(
      "const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())",
      "const __wasmFile =\n" +
        "  globalThis.__SATTERI_WASM_MODULE__ ??\n" +
        "  (await fetch(__wasmUrl).then((res) => res.arrayBuffer()))",
    ),
);
