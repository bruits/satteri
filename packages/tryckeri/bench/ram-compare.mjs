/**
 * Memory usage benchmark for tryckeri.
 *
 * Spawns each scenario in its own process for isolated RSS measurements.
 * Run:  node bench/ram-compare.mjs
 */
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SELF = fileURLToPath(import.meta.url);

// ── Worker mode: run a single scenario ──────────────────────────────────────
if (process.env.RAM_BENCH_SCENARIO) {
  if (typeof globalThis.gc !== "function") {
    console.error("worker must run with --expose-gc");
    process.exit(1);
  }

  const { readFileSync } = await import("node:fs");
  const BASE_MD = readFileSync(new URL("./markdown.md", import.meta.url), "utf8");
  const scale = parseInt(process.env.RAM_BENCH_SCALE || "1", 10);
  const md =
    scale === 1 ? BASE_MD : Array.from({ length: scale }, () => BASE_MD).join("\n\n---\n\n");

  const scenario = process.env.RAM_BENCH_SCENARIO;
  const iterations = 200;
  const warmup = 50;

  const fn = await buildScenario(scenario, md);

  // warmup
  for (let i = 0; i < warmup; i++) fn();
  globalThis.gc();

  const before = process.memoryUsage();
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
    if (i % 10 === 9) globalThis.gc();
  }
  const elapsed = performance.now() - start;
  globalThis.gc();
  const after = process.memoryUsage();

  // Peak: no GC
  globalThis.gc();
  const peakBefore = process.memoryUsage();
  for (let i = 0; i < iterations; i++) fn();
  const peak = process.memoryUsage();
  globalThis.gc();

  const result = {
    msPerOp: elapsed / iterations,
    steadyRSS_MB: after.rss / 1024 / 1024,
    steadyHeap_MB: after.heapUsed / 1024 / 1024,
    steadyExt_MB: after.external / 1024 / 1024,
    peakExtKB: (peak.external - peakBefore.external) / 1024,
  };
  process.stdout.write(JSON.stringify(result));
  process.exit(0);
}

// ── Coordinator mode ────────────────────────────────────────────────────────

async function buildScenario(name, md) {
  const {
    parseToHtml,
    compileMarkdownToHtml,
    compileMdxToJs,
    defineHastPlugin,
    defineMdastPlugin,
  } = await import("../dist/index.js");

  switch (name) {
    case "pure-rust":
      return () => parseToHtml(md);

    case "html-no-plugins":
      return () => compileMarkdownToHtml(md);

    case "html-with-plugins": {
      const hast = defineHastPlugin({
        name: "add-class",
        createOnce: () => ({
          element: {
            filter: ["a", "h1", "h2", "h3"],
            visit(node, ctx) {
              ctx.setProperty(node, "class", "styled");
            },
          },
        }),
      });
      const mdast = defineMdastPlugin({
        name: "heading-depth",
        createOnce: () => ({
          heading(node, ctx) {
            if (node.depth === 1) ctx.setProperty(node, "depth", 2);
          },
        }),
      });
      return () => compileMarkdownToHtml(md, { mdastPlugins: [mdast], hastPlugins: [hast] });
    }

    case "mdx-no-plugins": {
      const mdxSafe = md.replace(/<[^>]+>/g, "");
      const mdx = `import {Chart} from './chart.js'\n\n${mdxSafe}\n\n<Chart values={[1, 2, 3]} />\n`;
      return () => compileMdxToJs(mdx);
    }

    case "mdx-with-plugins": {
      const mdxSafe = md.replace(/<[^>]+>/g, "");
      const mdx = `import {Chart} from './chart.js'\n\n${mdxSafe}\n\n<Chart values={[1, 2, 3]} />\n`;
      const hast = defineHastPlugin({
        name: "add-class",
        createOnce: () => ({
          element: {
            filter: ["a", "h1", "h2", "h3"],
            visit(node, ctx) {
              ctx.setProperty(node, "class", "styled");
            },
          },
        }),
      });
      const mdast = defineMdastPlugin({
        name: "heading-depth",
        createOnce: () => ({
          heading(node, ctx) {
            if (node.depth === 1) ctx.setProperty(node, "depth", 2);
          },
        }),
      });
      return () => compileMdxToJs(mdx, { mdastPlugins: [mdast], hastPlugins: [hast] });
    }

    default:
      throw new Error(`Unknown scenario: ${name}`);
  }
}

function runScenario(name, scale) {
  const out = execFileSync(process.execPath, ["--expose-gc", SELF], {
    env: { ...process.env, RAM_BENCH_SCENARIO: name, RAM_BENCH_SCALE: String(scale) },
    timeout: 120_000,
    maxBuffer: 10 * 1024 * 1024,
  });
  return JSON.parse(out.toString());
}

const col = (s, w) => String(s).padStart(w);
const lft = (s, w) => String(s).padEnd(w);

const scenarios = [
  ["parseToHtml (pure Rust)", "pure-rust"],
  ["HTML — no plugins", "html-no-plugins"],
  ["HTML — with plugins", "html-with-plugins"],
  ["MDX  — no plugins", "mdx-no-plugins"],
  ["MDX  — with plugins", "mdx-with-plugins"],
];

for (const scale of [1, 10]) {
  const label = scale === 1 ? "1x (~11KB)" : "10x (~112KB)";
  console.log(`\n${"=".repeat(90)}`);
  console.log(`Document: ${label}   |  200 iterations, GC every 10`);
  console.log(`${"=".repeat(90)}\n`);

  const hdr = [
    lft("Scenario", 35),
    col("ms/op", 9),
    col("heap MB", 10),
    col("ext MB", 9),
    col("RSS MB", 9),
    col("peak ext MB", 13),
  ].join("");
  console.log(hdr);
  console.log("-".repeat(hdr.length));

  for (const [label, id] of scenarios) {
    try {
      const r = runScenario(id, scale);
      console.log(
        lft(label, 35),
        col(r.msPerOp.toFixed(2), 9),
        col(r.steadyHeap_MB.toFixed(1), 10),
        col(r.steadyExt_MB.toFixed(1), 9),
        col(r.steadyRSS_MB.toFixed(1), 9),
        col((r.peakExtKB / 1024).toFixed(1), 13),
      );
    } catch (e) {
      console.log(lft(label, 35), `  ERROR: ${e.message.split("\n")[0]}`);
    }
  }
}

console.log("\nheap    = JS heap after GC (steady state)");
console.log("ext     = external/native memory after GC");
console.log("RSS     = resident set size (steady state)");
console.log("peak ext = native memory accumulated without GC (backpressure)");
