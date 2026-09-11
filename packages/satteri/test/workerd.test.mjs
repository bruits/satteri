import assert from "node:assert/strict";
import { execFile as execFileCallback } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "node:test";
import { NapiCli } from "@napi-rs/cli";
import { build } from "esbuild";
import { Miniflare } from "miniflare";

const execFile = promisify(execFileCallback);
const require = createRequire(import.meta.url);
const packageDir = fileURLToPath(new URL("../", import.meta.url));

test(
  "the packed package runs in workerd without Node compatibility",
  { timeout: 600_000 },
  async () => {
    const temporaryDir = await mkdtemp(join(tmpdir(), "satteri-workerd-"));
    let mf;
    try {
      const packageManager = process.env.npm_execpath;
      const pnpm = (cwd, ...args) =>
        execFile(
          packageManager ? process.execPath : "pnpm",
          [
            ...(packageManager ? [packageManager] : []),
            "--config.auto-install-peers=true",
            ...args,
          ],
          { cwd },
        );
      // Exercise the release preparation without modifying or publishing the real package.
      const stageDir = join(temporaryDir, "stage");
      const manifest = JSON.parse(await readFile(join(packageDir, "package.json"), "utf8"));
      await mkdir(stageDir);
      for (const file of [...manifest.files, "patch-binding.js"]) {
        await cp(join(packageDir, file), join(stageDir, file), { recursive: true });
      }
      // This test builds only the opt-in target; keep the real opt-in dependency policy.
      manifest.napi.targets = ["wasm32-wasip1"];
      await writeFile(join(stageDir, "package.json"), JSON.stringify(manifest));
      await cp(join(packageDir, "target/wasm32-wasip1"), join(stageDir, "artifacts"), {
        recursive: true,
      });
      const cli = new NapiCli();
      await cli.createNpmDirs({ cwd: stageDir });
      await cli.artifacts({ cwd: stageDir });
      const prepareRelease = () =>
        cli.prePublish({
          cwd: stageDir,
          tagStyle: "npm",
          skipOptionalPublish: true,
          ghRelease: false,
        });
      await prepareRelease();
      await execFile(process.execPath, ["patch-binding.js", "--package"], { cwd: stageDir });
      // Release preparation and the workaround must both be repeatable.
      const patchedManifest = await readFile(join(stageDir, "package.json"), "utf8");
      await prepareRelease();
      await execFile(process.execPath, ["patch-binding.js", "--package"], { cwd: stageDir });
      assert.equal(await readFile(join(stageDir, "package.json"), "utf8"), patchedManifest);
      const pack = async (cwd, destination) => {
        await mkdir(destination);
        await pnpm(cwd, "pack", "--pack-destination", destination);
        const archives = (await readdir(destination)).filter((file) => file.endsWith(".tgz"));
        assert.equal(archives.length, 1);
        return join(destination, archives[0]);
      };
      const mainArchive = await pack(stageDir, join(temporaryDir, "main-package"));
      const bindingArchive = await pack(
        join(stageDir, "npm/wasm32-wasip1"),
        join(temporaryDir, "binding-package"),
      );
      const listing = await execFile("tar", ["-tzf", mainArchive]);
      assert.doesNotMatch(listing.stdout, /\.wasm(?:\n|$)/);
      assert.doesNotMatch(listing.stdout, /dist\/workerd\//);
      const published = JSON.parse(patchedManifest);
      assert.equal(published.exports["./wasm.wasm"], undefined);
      assert.equal(published.exports["./wasm"], undefined);
      assert.equal(published.exports["./workerd"], undefined);
      assert.equal(published.optionalDependencies["@bruits/satteri-wasm32-wasip1"], undefined);
      assert.equal(published.peerDependencies["@bruits/satteri-wasm32-wasip1"], published.version);
      assert.equal(published.peerDependenciesMeta["@bruits/satteri-wasm32-wasip1"].optional, true);

      // Real isolated pnpm installs verify that the optional peer is not downloaded by
      // Node-only users, but is resolvable from satteri when explicitly installed.
      await writeFile(join(temporaryDir, "package.json"), '{"private":true,"type":"module"}');
      await pnpm(temporaryDir, "add", "--ignore-scripts", "--prefer-offline", mainArchive);
      assert.ok(
        (await readdir(join(temporaryDir, "node_modules/.pnpm"))).every(
          (entry) => !entry.startsWith("@bruits+satteri-wasm32-wasip1@"),
        ),
      );
      const consumerRequire = createRequire(join(temporaryDir, "package.json"));
      assert.throws(() => consumerRequire.resolve("@bruits/satteri-wasm32-wasip1/package.json"), {
        code: "MODULE_NOT_FOUND",
      });
      await writeFile(
        join(temporaryDir, "consumer.ts"),
        `
      import { markdownToHtml, createMdastHandle, serializeHandle, dropHandle } from "satteri";
      const html: string = markdownToHtml("# Hello").html;
      const handle = createMdastHandle(html);
      const bytes: Uint8Array = serializeHandle(handle);
      dropHandle(handle);
    `,
      );
      await writeFile(
        join(temporaryDir, "tsconfig.json"),
        JSON.stringify({
          compilerOptions: {
            target: "es2022",
            module: "esnext",
            moduleResolution: "bundler",
            customConditions: ["workerd"],
            lib: ["ES2022", "DOM"],
            types: [],
            strict: true,
            noEmit: true,
          },
          files: ["consumer.ts"],
        }),
      );
      await execFile(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", temporaryDir]);

      await pnpm(temporaryDir, "add", "--ignore-scripts", "--prefer-offline", bindingArchive);
      await writeFile(
        join(temporaryDir, "worker.js"),
        await readFile(new URL("./fixtures/workerd-worker.js", import.meta.url)),
      );
      const { outputFiles } = await build({
        absWorkingDir: temporaryDir,
        entryPoints: ["worker.js"],
        bundle: true,
        write: false,
        outfile: join(temporaryDir, "bundle.js"),
        assetNames: "[name]",
        format: "esm",
        platform: "browser",
        conditions: ["workerd", "browser"],
        loader: { ".wasm": "copy" },
      });
      mf = new Miniflare({
        compatibilityDate: "2026-05-01",
        // No nodejs_compat, unsafe dynamic-Wasm/eval options, or Buffer polyfill.
        modulesRoot: temporaryDir,
        modules: outputFiles
          .toSorted((a, b) => Number(a.path.endsWith(".wasm")) - Number(b.path.endsWith(".wasm")))
          .map((file) => ({
            type: file.path.endsWith(".wasm") ? "CompiledWasm" : "ESModule",
            path: file.path,
            contents: file.path.endsWith(".wasm") ? file.contents : file.text,
          })),
      });

      // Reuse the isolate after parse errors and asynchronous visitors on both trees.
      for (let i = 1; i <= 3; i++) {
        const response = await mf.dispatchFetch("http://localhost/", {
          method: "POST",
          body: `# Request ${i}\n\nHello **workerd** — café 🦀`,
        });
        const text = await response.text();
        assert.equal(response.status, 200, text);
        const result = JSON.parse(text);
        assert.equal(result.requests, i);
        assert.equal(result.hasBuffer, false);
        assert.equal(
          result.html,
          `<h1>Request ${i}</h1>\n<p>Hello <strong>workerd</strong> — café 🦀</p>\n`,
        );
        assert.equal(result.plugins, "<h1>Hello!</h1>\n<p>Use CODE.</p>\n");
        assert.equal(result.mdast.type, "root");
        assert.equal(result.mdast.children[0].children[0].type, "strong");
        assert.equal(result.mdast.children[0].children[0].children[0].value, "Hello");
        assert.equal(result.hast.type, "root");
        assert.equal(result.hast.children[0].tagName, "p");
        assert.equal(result.hast.children[0].children[0].tagName, "strong");
        assert.equal(result.hast.children[0].children[0].children[0].value, "Hello");
        assert.equal(result.htmlRoundTrip, "<p>Hello <em>workerd</em></p>");
        assert.match(result.markdownJs, /export default MDXContent/);
        assert.match(result.mdxJs, /export const answer = 42/);
        assert.match(result.mdxJs, /Widget/);
        assert.equal(typeof result.invalidMdx, "string");
        assert.ok(result.invalidMdx.length > 0);
        assert.equal(result.recovered, "<p>Recovered</p>\n");
      }
    } finally {
      await mf?.dispose();
      await rm(temporaryDir, { recursive: true, force: true });
    }
  },
);
