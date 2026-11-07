import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { Miniflare } from "miniflare";

const packageDir = fileURLToPath(new URL("../", import.meta.url));
const bundleDir = join(packageDir, "workerd-bundle");

test("runs in workerd without Node compatibility", async () => {
  const { outputFiles } = await build({
    entryPoints: [fileURLToPath(new URL("./fixtures/workerd-worker.js", import.meta.url))],
    bundle: true,
    write: false,
    outdir: bundleDir,
    assetNames: "[name]",
    format: "esm",
    platform: "browser",
    conditions: ["workerd", "worker", "browser"],
    // The WASI package only exists after `napi artifacts`, so resolve it to the local build.
    alias: {
      satteri: packageDir,
      "@bruits/satteri-wasm32-wasip1/workerd": join(packageDir, "satteri_napi.wasip1-deferred.js"),
      "@bruits/satteri-wasm32-wasip1/wasm.wasm": join(
        packageDir,
        "satteri_napi.wasm32-wasip1.wasm",
      ),
    },
    loader: { ".wasm": "copy" },
  });
  const worker = outputFiles.find((file) => file.path.endsWith(".js"));
  const wasm = outputFiles.find((file) => file.path.endsWith(".wasm"));
  const mf = new Miniflare({
    compatibilityDate: "2026-05-01",
    modulesRoot: bundleDir,
    modules: [
      { type: "ESModule", path: worker.path, contents: worker.text },
      { type: "CompiledWasm", path: wasm.path, contents: wasm.contents },
    ],
  });

  try {
    for (let i = 1; i <= 3; i++) {
      const response = await mf.dispatchFetch("http://localhost/", {
        method: "POST",
        body: `# Request ${i}\n\nHello **workerd**, café 🦀`,
      });
      const text = await response.text();
      assert.equal(response.status, 200, text);
      const result = JSON.parse(text);
      assert.equal(result.requests, i);
      assert.equal(result.hasBuffer, false);
      assert.equal(
        result.html,
        `<h1>Request ${i}</h1>\n<p>Hello <strong>workerd</strong>, café 🦀</p>\n`,
      );
      assert.equal(result.plugins, "<h1>Hello!</h1>\n<p>Use CODE.</p>\n");
      assert.equal(result.mdast.children[0].children[0].type, "strong");
      assert.equal(result.hast.children[0].children[0].tagName, "strong");
      assert.equal(result.htmlRoundTrip, "<p>Hello <em>workerd</em></p>");
      assert.match(result.markdownJs, /export default MDXContent/);
      assert.match(result.mdxJs, /export const answer = 42/);
      assert.ok(result.invalidMdx.length > 0);
      assert.equal(result.recovered, "<p>Recovered</p>\n");
    }
  } finally {
    await mf.dispose();
  }
});
