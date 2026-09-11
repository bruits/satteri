// Compare release addons built with different wire_out implementations:
// node --expose-gc bench/wire-transfer.mjs /path/buffer.node /path/uint8.node
// Each addon runs in a fresh process; alternate their order across three rounds.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { cpus } from "node:os";
import { resolve } from "node:path";
import { setImmediate } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(import.meta.url);
const quantile = (values, fraction) => {
  const sorted = values.toSorted((a, b) => a - b);
  return sorted[Math.floor((sorted.length - 1) * fraction)];
};

if (process.argv[2] !== "--worker") {
  const addons = process.argv.slice(2).map((path) => resolve(path));
  assert.ok(addons.length > 0, "Pass release .node addon paths to compare");
  const results = [];
  for (let round = 0; round < 3; round++) {
    for (const addon of round % 2 ? addons.toReversed() : addons) {
      const child = spawnSync(process.execPath, ["--expose-gc", script, "--worker", addon], {
        encoding: "utf8",
        env: { ...process.env, NAPI_RS_NATIVE_LIBRARY_PATH: addon },
      });
      assert.equal(child.status, 0, child.stderr || child.stdout);
      results.push({ addon, round, cases: JSON.parse(child.stdout) });
    }
  }
  const summary = addons.flatMap((addon) => {
    const runs = results.filter((result) => result.addon === addon);
    return runs[0].cases.map((entry, i) => {
      const samples = runs.flatMap((run) => run.cases[i].samplesUs);
      return {
        addon,
        case: entry.name,
        wireBytes: entry.wireBytes,
        medianUs: quantile(samples, 0.5),
        p25Us: quantile(samples, 0.25),
        p75Us: quantile(samples, 0.75),
      };
    });
  });
  console.log(
    JSON.stringify({ node: process.version, cpu: cpus()[0].model, summary, results }, null, 2),
  );
} else {
  assert.equal(typeof globalThis.gc, "function", "Run with --expose-gc");
  const binding = createRequire(import.meta.url)(process.argv[3]);
  const api = await import("../dist/index.js");
  const sources = [
    ...[16, 512, 2048, 6000, 7600, 8200, 16384].map((size) => [`${size}B-text`, "a".repeat(size)]),
    ["document", readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8")],
  ];
  const results = [];
  let sink;
  function batch(fn, iterations) {
    const start = performance.now();
    for (let i = 0; i < iterations; i++) sink = fn();
    return performance.now() - start;
  }
  async function measure(name, wireBytes, fn) {
    batch(fn, 500);
    const calibration = batch(fn, 1000);
    const iterations = Math.max(100, Math.min(10000, Math.ceil(5000 / calibration)));
    const samplesUs = [];
    for (let sample = 0; sample < 9; sample++) {
      // Drain finalizers between samples so external allocations do not pile up.
      // Explicit GC is outside the timer; incidental GC inside a batch is included.
      sink = undefined;
      globalThis.gc();
      await setImmediate();
      samplesUs.push((batch(fn, iterations) * 1000) / iterations);
    }
    results.push({ name, wireBytes, samplesUs });
  }
  for (const [label, source] of sources) {
    for (const kind of ["mdast", "hast"]) {
      const create = kind === "mdast" ? api.createMdastHandle : api.createHastHandle;
      const parse = kind === "mdast" ? api.markdownToMdast : api.markdownToHast;
      const handle = create(source);
      try {
        const wire = binding.serializeHandle(handle);
        assert.ok(wire instanceof Uint8Array);
        assert.equal(parse(source).type, "root");
        if (label !== "document") {
          assert.equal(parse(source).children[0].children[0].value, source);
        }
        await measure(`${kind}/${label}/serialize`, wire.byteLength, () =>
          binding.serializeHandle(handle),
        );
        const parseWire =
          kind === "mdast"
            ? () => binding.parseMdastWire(source, 0, false, true)
            : () => binding.parseHastWire(source, 0, undefined, false, true);
        await measure(`${kind}/${label}/parse-wire`, wire.byteLength, parseWire);
        await measure(`${kind}/${label}/parse-tree`, wire.byteLength, () => parse(source));
      } finally {
        api.dropHandle(handle);
      }
    }
  }
  // Keep the last result observable to the optimizer without retaining every tree.
  assert.ok(sink);
  console.log(JSON.stringify(results));
}
