import { copyFileSync, readFileSync, writeFileSync } from "node:fs";

// napi-rs requires CommonJS declarations for WASI, while the ESM bridge and
// TypeScript sources still resolve index.d.ts.
if (process.argv.includes("--wasm")) {
  copyFileSync(new URL("./index.d.cts", import.meta.url), new URL("./index.d.ts", import.meta.url));
}

const bindingPath = new URL("./index.js", import.meta.url);
let content = readFileSync(bindingPath, "utf-8");

if (content.includes("webcontainer-fallback")) {
  process.exit(0);
}

content = content.replace(
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
);

writeFileSync(bindingPath, content);
