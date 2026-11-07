import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, test, expect } from "vitest";
import {
  createHastHandle,
  createMdastHandle,
  HastReader,
  materializeHastTree,
  materializeMdastTree,
  MdastReader,
  serializeHandle,
} from "../src/index.js";

const packageUrl = new URL("../", import.meta.url);
const packageDir = fileURLToPath(packageUrl);
const source = "# Hi *there*\n\n- [x] done\n\n<div>raw</div>\n";

// Runs in a fresh process where resolving the binding, native or WASI, throws, so these imports
// pass only if none of the subpath modules reach it.
function materializeWithoutBinding(mdastWire: Uint8Array, hastWire: Uint8Array) {
  const script = `
    import { registerHooks } from "node:module";
    const bindingEntries = ["index.js", "dist/binding.js", "dist/binding.browser.js"].map(
      (path) => new URL(path, ${JSON.stringify(packageUrl.href)}).href,
    );
    registerHooks({
      resolve(specifier, context, next) {
        const result = next(specifier, context);
        if (bindingEntries.includes(result.url)) throw new Error("the binding was loaded");
        return result;
      },
    });
    const { MdastReader } = await import("satteri/mdast-reader");
    const { materializeMdastTree } = await import("satteri/mdast-materializer");
    const { HastReader } = await import("satteri/hast-reader");
    const { materializeHastTree } = await import("satteri/hast-materializer");
    const wire = (base64) => new Uint8Array(Buffer.from(base64, "base64"));
    process.stdout.write(JSON.stringify({
      mdast: materializeMdastTree(new MdastReader(wire(${JSON.stringify(Buffer.from(mdastWire).toString("base64"))}))),
      hast: materializeHastTree(new HastReader(wire(${JSON.stringify(Buffer.from(hastWire).toString("base64"))}))),
    }));
  `;
  return spawnSync(process.execPath, ["--input-type=module", "-e", script], {
    cwd: packageDir,
    encoding: "utf8",
  });
}

describe("reader and materializer subpath exports", () => {
  test("materialize a serialized tree without loading the binding", () => {
    const mdastWire = serializeHandle(createMdastHandle(source));
    const hastWire = serializeHandle(createHastHandle(source));
    const result = materializeWithoutBinding(mdastWire, hastWire);
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toEqual(
      JSON.parse(
        JSON.stringify({
          mdast: materializeMdastTree(new MdastReader(mdastWire)),
          hast: materializeHastTree(new HastReader(hastWire)),
        }),
      ),
    );
  });
});
