/**
 * Memory benchmark — measures heap allocation per pipeline path.
 *
 * Run:  node --expose-gc bench/memory.mjs
 */
import { readFileSync } from "node:fs";
import {
  compileMarkdownToHtml,
  compileMdxToJs,
  defineMdastPlugin,
  defineHastPlugin,
} from "../dist/index.js";

const ITERATIONS = 100;
const WARMUP = 20;

const BASE_MD = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");
const LARGE_MD = Array.from({ length: 10 }, () => BASE_MD).join("\n\n---\n\n");
const MDX_SAFE_MD = LARGE_MD.replace(/<[^>]+>/g, "");
const LARGE_MDX = `import {Chart} from './chart.js'\n\n${MDX_SAFE_MD}\n\n<Chart values={[1, 2, 3]} />\n`;

const hasGc = typeof globalThis.gc === "function";
if (!hasGc) console.log("Warning: run with --expose-gc for accurate results\n");

console.log(
  `Doc sizes:  MD ${(LARGE_MD.length / 1024).toFixed(0)} KB  MDX ${(LARGE_MDX.length / 1024).toFixed(0)} KB`,
);
console.log(`Iterations: ${ITERATIONS}  Warmup: ${WARMUP}\n`);

// -- Plugins ----------------------------------------------------------------

const noopMdast = defineMdastPlugin({
  name: "noop-mdast",
  createOnce: () => ({ heading() {} }),
});
const noopHast = defineHastPlugin({
  name: "noop-hast",
  createOnce: () => ({ element: { filter: [], visit() {} } }),
});
const mutatingMdast = defineMdastPlugin({
  name: "mutate-mdast",
  createOnce: () => ({
    heading(node, ctx) {
      ctx.setProperty(node, "depth", 2);
    },
  }),
});
const mutatingHast = defineHastPlugin({
  name: "mutate-hast",
  createOnce: () => ({
    element: {
      filter: [],
      visit(node, ctx) {
        ctx.setProperty(node, "class", "bench");
      },
    },
  }),
});

// -- Measure ----------------------------------------------------------------

function gc() {
  if (hasGc) globalThis.gc();
}

function measure(name, fn) {
  // warmup
  for (let i = 0; i < WARMUP; i++) fn();

  gc();
  const before = process.memoryUsage();

  const start = performance.now();
  for (let i = 0; i < ITERATIONS; i++) fn();
  const elapsed = performance.now() - start;

  const after = process.memoryUsage();
  gc();
  const afterGc = process.memoryUsage();

  const KB = 1024;
  const MB = 1024 * 1024;
  return {
    name,
    msPerOp: elapsed / ITERATIONS,
    heapDeltaKB: (after.heapUsed - before.heapUsed) / KB,
    heapGcKB: (afterGc.heapUsed - before.heapUsed) / KB,
    extDeltaKB: (after.external - before.external) / KB,
    arrBufKB: (after.arrayBuffers - before.arrayBuffers) / KB,
    rssMB: process.memoryUsage.rss() / MB,
  };
}

// -- Scenarios --------------------------------------------------------------

// Filtered (selective) plugins — Rust-side walk
// Unfiltered element() that manually checks tagName — same work as filtered
const unfilteredAOnly = defineHastPlugin({
  name: "unfiltered-a-only",
  createOnce: () => ({
    element: {
      filter: ["a"],
      visit(node, ctx) {
        ctx.setProperty(node, "class", "link");
      },
    },
  }),
});

// Same as mutatingHast but using filter API (empty filter = all elements)
const mutatingHastFiltered = defineHastPlugin({
  name: "mutate-hast-filtered",
  createOnce: () => ({
    element: {
      filter: [],
      visit(node, ctx) {
        ctx.setProperty(node, "class", "bench");
      },
    },
  }),
});

const filteredHast = defineHastPlugin({
  name: "filtered-hast",
  createOnce: () => ({
    element: {
      filter: ["a"],
      visit(node, ctx) {
        ctx.setProperty(node, "class", "link");
      },
    },
  }),
});

const filteredHastMulti = defineHastPlugin({
  name: "filtered-multi",
  createOnce: () => ({
    element: [
      {
        filter: ["a"],
        visit(node, ctx) {
          ctx.setProperty(node, "class", "link");
        },
      },
      {
        filter: ["h1", "h2", "h3"],
        visit(node, ctx) {
          ctx.setProperty(node, "id", "heading");
        },
      },
    ],
  }),
});

const scenarios = [
  ["HTML — pure Rust (no plugins)", () => compileMarkdownToHtml(LARGE_MD)],
  ["HTML — no plugins", () => compileMarkdownToHtml(LARGE_MD)],
  [
    "HTML — noop mdast plugin",
    () => compileMarkdownToHtml(LARGE_MD, { mdastPlugins: [noopMdast] }),
  ],
  ["HTML — noop hast plugin", () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [noopHast] })],
  [
    "HTML — mutating mdast",
    () => compileMarkdownToHtml(LARGE_MD, { mdastPlugins: [mutatingMdast] }),
  ],
  [
    "HTML — mutating hast (bare fn)",
    () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [mutatingHast] }),
  ],
  [
    "HTML — mutating hast (filter)",
    () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [mutatingHastFiltered] }),
  ],
  [
    "HTML — element() if a",
    () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [unfilteredAOnly] }),
  ],
  ["HTML — filter: [a]", () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [filteredHast] })],
  [
    "HTML — filtered hast [a,h1-h3]",
    () => compileMarkdownToHtml(LARGE_MD, { hastPlugins: [filteredHastMulti] }),
  ],
  [
    "HTML — both (mutating)",
    () =>
      compileMarkdownToHtml(LARGE_MD, {
        mdastPlugins: [mutatingMdast],
        hastPlugins: [mutatingHast],
      }),
  ],
  ["MDX  — pure Rust (no plugins)", () => compileMdxToJs(LARGE_MDX)],
  ["MDX  — no plugins", () => compileMdxToJs(LARGE_MDX)],
  ["MDX  — noop mdast plugin", () => compileMdxToJs(LARGE_MDX, { mdastPlugins: [noopMdast] })],
  ["MDX  — noop hast plugin", () => compileMdxToJs(LARGE_MDX, { hastPlugins: [noopHast] })],
  [
    "MDX  — both (mutating)",
    () => compileMdxToJs(LARGE_MDX, { mdastPlugins: [mutatingMdast], hastPlugins: [mutatingHast] }),
  ],
];

// -- Run & print ------------------------------------------------------------

const col = (s, w) => String(s).padStart(w);
const lft = (s, w) => String(s).padEnd(w);

const W = { name: 36, n: 9, kb: 12 };
const hdr = [
  lft("Scenario", W.name),
  col("ms/op", W.n),
  col("heap Δ KB", W.kb),
  col("heap GC KB", W.kb),
  col("ext Δ KB", W.kb),
  col("arrBuf KB", W.kb),
  col("RSS MB", W.kb),
].join("");
console.log(hdr);
console.log("-".repeat(hdr.length));

for (const [name, fn] of scenarios) {
  try {
    const r = measure(name, fn);
    console.log(
      lft(r.name, W.name),
      col(r.msPerOp.toFixed(2), W.n),
      col(r.heapDeltaKB.toFixed(0), W.kb),
      col(r.heapGcKB.toFixed(0), W.kb),
      col(r.extDeltaKB.toFixed(0), W.kb),
      col(r.arrBufKB.toFixed(0), W.kb),
      col(r.rssMB.toFixed(1), W.kb),
    );
  } catch (e) {
    console.log(lft(name, W.name), `  SKIP: ${e.message.split("\n")[0]}`);
  }
}

console.log("\nheap Δ = heap growth during 100 iterations (before GC)");
console.log("heap GC = heap growth retained after GC (leaks show here)");
console.log("ext Δ = external/native memory growth");
console.log("RSS = process resident set at end of scenario");
