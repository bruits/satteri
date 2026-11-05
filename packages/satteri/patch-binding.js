import { readFileSync, writeFileSync } from "node:fs";

// pre-publish copies the workerd Wasm into the root package, but #binding loads it from the WASI package.
if (process.argv.includes("--strip-root-wasm")) {
  const manifestPath = new URL("./package.json", import.meta.url);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf-8"));
  manifest.files = manifest.files.filter((file) => !file.startsWith("satteri_napi.wasm32-wasip1."));
  for (const subpath of ["./workerd", "./wasm", "./wasm.wasm"]) delete manifest.exports[subpath];
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
  process.exit(0);
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
