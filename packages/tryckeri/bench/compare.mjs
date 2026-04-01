/**
 * Quick A/B comparison script. Works with both sync and async versions.
 * Run: node bench/compare.mjs
 */
import { compileMarkdownToHtml, defineHastPlugin } from "../dist/index.js";
import { parseToHtml } from "../dist/index.js";
import { readFileSync } from "node:fs";

const MARKDOWN = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");

const ITERATIONS = 2000;
const WARMUP = 200;

async function bench(name, fn) {
  // warmup
  for (let i = 0; i < WARMUP; i++) await fn();

  const start = performance.now();
  for (let i = 0; i < ITERATIONS; i++) await fn();
  const elapsed = performance.now() - start;

  const opsPerSec = (ITERATIONS / elapsed) * 1000;
  const meanMs = elapsed / ITERATIONS;
  console.log(
    `${name.padEnd(55)} ${opsPerSec.toFixed(0).padStart(8)} ops/s   ${meanMs.toFixed(4).padStart(8)} ms/op`,
  );
}

console.log(`\n--- Benchmark (${ITERATIONS} iterations, ${WARMUP} warmup) ---\n`);

await bench("parseToHtml (pure Rust baseline)", () => parseToHtml(MARKDOWN));

await bench("compileMarkdownToHtml — no plugins", () => compileMarkdownToHtml(MARKDOWN));

const noopPlugin = defineHastPlugin({
  name: "sync-noop",
  createOnce: () => ({
    element() {},
  }),
});
await bench("compileMarkdownToHtml — sync noop plugin", () =>
  compileMarkdownToHtml(MARKDOWN, { hastPlugins: [noopPlugin] }));

console.log("");
