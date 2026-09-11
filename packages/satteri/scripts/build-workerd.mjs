import { NapiCli } from "@napi-rs/cli";
import { build } from "esbuild";
import { copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = fileURLToPath(new URL("../", import.meta.url));
const outputDir = join(packageDir, "dist/workerd");
// Keep intermediates near node_modules for emnapi resolution, but out of the package.
const temporaryDir = await mkdtemp(join(packageDir, ".workerd-"));

try {
  const { task } = await new NapiCli().build({
    cwd: packageDir,
    manifestPath: "../../crates/satteri-napi-binding/Cargo.toml",
    outputDir: temporaryDir,
    platform: true,
    release: true,
    esm: true,
    strip: true,
    target: "wasm32-wasip1",
    dts: "index.d.cts",
  });
  await task;

  await rm(outputDir, { recursive: true, force: true });
  await mkdir(outputDir, { recursive: true });
  await build({
    absWorkingDir: packageDir,
    entryPoints: ["src/index.ts"],
    outfile: join(outputDir, "index.js"),
    alias: {
      "#binding": fileURLToPath(new URL("./workerd-binding.js", import.meta.url)),
      "#workerd-loader": join(temporaryDir, "satteri_napi.wasip1-deferred.js"),
    },
    bundle: true,
    minify: true,
    format: "esm",
    platform: "browser",
    conditions: ["workerd"],
    target: "es2022",
    plugins: [
      {
        name: "compiled-wasm",
        setup(builder) {
          builder.onResolve({ filter: /^#workerd-wasm$/ }, () => ({
            path: "./satteri.wasm",
            external: true,
          }));
        },
      },
    ],
  });
  await copyFile(
    join(temporaryDir, "satteri_napi.wasm32-wasip1.wasm"),
    join(outputDir, "satteri.wasm"),
  );
} finally {
  await rm(temporaryDir, { recursive: true, force: true });
}
