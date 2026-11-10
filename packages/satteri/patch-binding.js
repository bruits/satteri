import { readFileSync, writeFileSync } from "node:fs";

if (process.argv.includes("--pre-publish")) {
  const manifestPath = new URL("./package.json", import.meta.url);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf-8"));
  // pre-publish copies the workerd Wasm into the root package, but #binding loads it from the WASI package.
  manifest.files = manifest.files.filter((file) => !file.startsWith("satteri_napi.wasm32-wasip1."));
  for (const subpath of ["./workerd", "./wasm", "./wasm.wasm"]) delete manifest.exports[subpath];
  // An optional peer lets strict package managers resolve the WASI package when the user installs it.
  const wasiPackage = "@bruits/satteri-wasm32-wasip1";
  manifest.peerDependencies = { ...manifest.peerDependencies, [wasiPackage]: manifest.version };
  manifest.peerDependenciesMeta = {
    ...manifest.peerDependenciesMeta,
    [wasiPackage]: { optional: true },
  };
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
