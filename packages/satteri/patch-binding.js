import { copyFileSync, readFileSync, rmSync, writeFileSync } from "node:fs";

if (process.argv.includes("--package")) {
  const packagePath = new URL("./package.json", import.meta.url);
  const pkg = JSON.parse(readFileSync(packagePath, "utf8"));
  // NAPI's pre-publish adds root WASM facades even for an opt-in binding.
  // Keep the binary exclusively in the platform package until NAPI offers an opt-out.
  const prefix = `${pkg.napi.binaryName}.wasm32-wasip1.`;
  pkg.files = pkg.files.filter((file) => {
    if (!file.startsWith(prefix)) return true;
    rmSync(new URL(file, import.meta.url), { force: true });
    return false;
  });
  for (const subpath of ["./workerd", "./wasm", "./wasm.wasm"]) {
    if (pkg.exports[subpath]?.default?.startsWith(`./${prefix}`)) {
      delete pkg.exports[subpath];
    }
  }
  const binding = `${pkg.napi.packageName}-wasm32-wasip1`;
  pkg.peerDependencies = { ...pkg.peerDependencies, [binding]: pkg.version };
  pkg.peerDependenciesMeta = { ...pkg.peerDependenciesMeta, [binding]: { optional: true } };
  writeFileSync(packagePath, JSON.stringify(pkg, null, 2) + "\n");
  process.exit(0);
}

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
