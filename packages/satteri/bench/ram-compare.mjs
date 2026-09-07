/**
 * Memory usage benchmark for satteri.
 *
 * Spawns each scenario in its own process for isolated RSS measurements.
 * Run:  node bench/ram-compare.mjs
 */
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SELF = fileURLToPath(import.meta.url);

if (process.env.RAM_BENCH_SCENARIO) {
  if (typeof globalThis.gc !== "function") {
    console.error("worker must run with --expose-gc");
    process.exit(1);
  }

  const { readFileSync } = await import("node:fs");
  const BASE_MD = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");
  const scale = parseInt(process.env.RAM_BENCH_SCALE || "1", 10);
  const md =
    scale === 1 ? BASE_MD : Array.from({ length: scale }, () => BASE_MD).join("\n\n---\n\n");

  const scenario = process.env.RAM_BENCH_SCENARIO;
  const iterations = parseInt(process.env.RAM_BENCH_ITERATIONS || "200", 10);
  const warmup = Math.min(50, Math.floor(iterations / 4));

  const fn = await buildScenario(scenario, md);

  // warmup
  for (let i = 0; i < warmup; i++) fn();
  globalThis.gc();

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const elapsed = performance.now() - start;
  const after = process.memoryUsage();
  globalThis.gc();
  const afterGc = process.memoryUsage();

  const result = {
    msPerOp: elapsed / iterations,
    peakRSS_MB: after.rss / 1024 / 1024,
    peakHeap_MB: after.heapUsed / 1024 / 1024,
    peakExt_MB: after.external / 1024 / 1024,
    gcRSS_MB: afterGc.rss / 1024 / 1024,
    gcHeap_MB: afterGc.heapUsed / 1024 / 1024,
  };
  process.stdout.write(JSON.stringify(result));
  process.exit(0);
}

async function buildScenario(name, markdown) {
  const { markdownToHtml, mdxToJs, defineHastPlugin, defineMdastPlugin } =
    await import("../dist/index.js");

  if (name === "pure-rust") {
    const { markdownToHtmlFast } = await import("../index.js");
    const { DEFAULT_PARSE_OPTIONS } = await import("../dist/compile.js");
    return () => markdownToHtmlFast(markdown, DEFAULT_PARSE_OPTIONS);
  }

  const mdx = name.startsWith("mdx-");
  const source = mdx
    ? `import {Chart} from './chart.js'\n\n${markdown.replace(/<[^>]+>/g, "")}\n\n<Chart values={[1, 2, 3]} />\n`
    : markdown;
  const compile = mdx ? mdxToJs : markdownToHtml;
  const scenario = name.slice(name.indexOf("-") + 1);
  if (scenario === "no-plugins") return () => compile(source);

  const demoteHeadings = defineMdastPlugin({
    name: "demote-h1",
    heading(node, ctx) {
      if (node.depth === 1) ctx.setProperty(node, "depth", 2);
    },
  });
  const setProperty = (name, filter, key, value) =>
    defineHastPlugin({
      name,
      element: {
        filter,
        visit(node, ctx) {
          ctx.setProperty(node, key, value);
        },
      },
    });

  if (scenario === "with-plugins") {
    const options = {
      mdastPlugins: [demoteHeadings],
      hastPlugins: [setProperty("add-class", ["a", "h1", "h2", "h3"], "class", "styled")],
    };
    return () => compile(source, options);
  }
  if (scenario !== "many-plugins") throw new Error(`Unknown scenario: ${name}`);

  const mdastPlugins = [
    demoteHeadings,
    defineMdastPlugin(
      mdx
        ? {
            name: "mark-code",
            code(node, ctx) {
              ctx.setProperty(node, "data", { highlighted: true });
            },
          }
        : {
            name: "mark-images",
            image(node, ctx) {
              ctx.setProperty(node, "data", { processed: true });
            },
          },
    ),
    defineMdastPlugin({
      name: "strip-html",
      html(node, ctx) {
        ctx.removeNode(node);
      },
    }),
  ];
  const hastPlugins = [
    setProperty("heading-ids", ["h1", "h2", "h3", "h4", "h5", "h6"], "id", "slug"),
    ...(mdx
      ? [
          setProperty("link-target", ["a"], "target", "_blank"),
          setProperty("code-wrap", ["pre"], "class", "code-block"),
          defineHastPlugin({ name: "esm-tracker", mdxjsEsm() {} }),
        ]
      : [
          setProperty("link-class", ["a"], "class", "link"),
          setProperty("code-highlight", ["code"], "class", "highlight"),
          setProperty("img-lazy", ["img"], "loading", "lazy"),
        ]),
  ];
  const options = { mdastPlugins, hastPlugins };
  return () => compile(source, options);
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
  ["markdownToHtmlFast (pure Rust)", "pure-rust"],
  ["HTML - no plugins", "html-no-plugins"],
  ["HTML - 2 plugins (1+1)", "html-with-plugins"],
  ["HTML - 7 plugins (3+4)", "html-many-plugins"],
  ["MDX  - no plugins", "mdx-no-plugins"],
  ["MDX  - 2 plugins (1+1)", "mdx-with-plugins"],
  ["MDX  - 7 plugins (3+4)", "mdx-many-plugins"],
];

const SCALES = process.env.RAM_BENCH_SCALES
  ? process.env.RAM_BENCH_SCALES.split(",").map(Number)
  : [1, 10];

for (const scale of SCALES) {
  const label = `${scale}x (~${Math.round(scale * 11)}KB)`;
  console.log(`\n${"=".repeat(90)}`);
  const iters = process.env.RAM_BENCH_ITERATIONS || "200";
  console.log(`Document: ${label}   |  ${iters} iterations, no forced GC`);
  console.log(`${"=".repeat(90)}\n`);

  const hdr = [
    lft("Scenario", 35),
    col("ms/op", 9),
    col("peak RSS", 11),
    col("after GC", 11),
    col("peak heap", 11),
  ].join("");
  console.log(hdr);
  console.log("-".repeat(hdr.length));

  for (const [label, id] of scenarios) {
    try {
      const r = runScenario(id, scale);
      console.log(
        lft(label, 35),
        col(r.msPerOp.toFixed(2), 9),
        col(r.peakRSS_MB.toFixed(1) + " MB", 11),
        col(r.gcRSS_MB.toFixed(1) + " MB", 11),
        col(r.peakHeap_MB.toFixed(1) + " MB", 11),
      );
    } catch (e) {
      console.log(lft(label, 35), `  ERROR: ${e.message.split("\n")[0]}`);
    }
  }
}

console.log("\npeak RSS  = RSS at end of all iterations (before GC)");
console.log("after GC  = RSS after forcing GC (true retained)");
console.log("peak heap = JS heap at end of iterations");
