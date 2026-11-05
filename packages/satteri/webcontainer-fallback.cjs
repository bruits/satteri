// Adapted from https://github.com/rolldown/rolldown/blob/main/packages/rolldown/src/webcontainer-fallback.cjs

const fs = require("node:fs");
const childProcess = require("node:child_process");

const pkg = require("./package.json");
const version = pkg.version;

const baseDir = `/tmp/satteri-${version}`;
const bindingEntry = `${baseDir}/node_modules/@bruits/satteri-wasm32-wasip1/satteri_napi.wasip1.cjs`;

if (!fs.existsSync(bindingEntry)) {
  const bindingPkg = `@bruits/satteri-wasm32-wasip1@${version}`;
  fs.rmSync(baseDir, { recursive: true, force: true });
  fs.mkdirSync(baseDir, { recursive: true });
  console.log(`[satteri] Downloading ${bindingPkg} on WebContainer...`);
  childProcess.execFileSync("pnpm", ["i", bindingPkg], {
    cwd: baseDir,
    stdio: "inherit",
  });
}

module.exports = require(bindingEntry);
