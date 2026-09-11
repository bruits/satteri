import assert from "node:assert/strict";
import { execFile as execFileCallback } from "node:child_process";
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "node:test";
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
      // Exercise prepack, file inclusion, and consumer-side export resolution.
      // The consumer is outside the repo and has no native addon or runtime dependencies.
      const packageManager = process.env.npm_execpath;
      await execFile(
        packageManager ? process.execPath : "pnpm",
        [...(packageManager ? [packageManager] : []), "pack", "--pack-destination", temporaryDir],
        { cwd: packageDir },
      );
      const archives = (await readdir(temporaryDir)).filter((file) => file.endsWith(".tgz"));
      assert.equal(archives.length, 1);
      const installedDir = join(temporaryDir, "node_modules/satteri");
      await mkdir(installedDir, { recursive: true });
      await execFile("tar", [
        "-xzf",
        join(temporaryDir, archives[0]),
        "-C",
        installedDir,
        "--strip-components=1",
      ]);
      // Resolve declarations with only the package's declared type dependencies,
      // not the repository's ambient Node types or development dependencies.
      const manifest = JSON.parse(await readFile(join(installedDir, "package.json"), "utf8"));
      for (const dependency of Object.keys(manifest.dependencies)) {
        const destination = join(temporaryDir, "node_modules", dependency);
        await mkdir(dirname(destination), { recursive: true });
        await symlink(
          dirname(require.resolve(`${dependency}/package.json`)),
          destination,
          "junction",
        );
      }
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

      const wasmDir = join(installedDir, "dist/workerd");
      assert.deepEqual((await readdir(wasmDir)).toSorted(), ["index.js", "satteri.wasm"]);
      await writeFile(
        join(temporaryDir, "worker.js"),
        await readFile(new URL("./fixtures/workerd-worker.js", import.meta.url)),
      );
      const { outputFiles } = await build({
        absWorkingDir: temporaryDir,
        entryPoints: ["worker.js"],
        bundle: true,
        write: false,
        format: "esm",
        platform: "browser",
        conditions: ["workerd", "browser"],
        external: ["*.wasm"],
      });
      mf = new Miniflare({
        compatibilityDate: "2026-05-01",
        // No nodejs_compat, unsafe dynamic-Wasm/eval options, or Buffer polyfill.
        modulesRoot: temporaryDir,
        modules: [
          {
            type: "ESModule",
            path: join(temporaryDir, "bundle.js"),
            contents: outputFiles[0].text,
          },
          {
            type: "CompiledWasm",
            path: join(temporaryDir, "satteri.wasm"),
            contents: await readFile(join(wasmDir, "satteri.wasm")),
          },
        ],
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
