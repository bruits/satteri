/**
 * Profile where time is spent in the plugin path.
 */
import {
  createMdastHandle,
  createHastHandle,
  convertMdastToHastHandle,
  serializeHandle,
  renderHandle,
  dropHandle,
} from "../index.js";
import { HastReader } from "../dist/hast/hast-reader.js";
import { visitHast } from "../dist/hast/hast-visitor.js";
import { DataMap } from "../dist/data-map.js";
import { readFileSync } from "node:fs";

const MARKDOWN = readFileSync(new URL("./fixtures/markdown.md", import.meta.url), "utf8");
const ITERATIONS = 2000;
const WARMUP = 200;

function time(name, fn) {
  for (let i = 0; i < WARMUP; i++) fn();
  const start = performance.now();
  for (let i = 0; i < ITERATIONS; i++) fn();
  const elapsed = performance.now() - start;
  const mean = elapsed / ITERATIONS;
  console.log(`${name.padEnd(55)} ${mean.toFixed(4).padStart(8)} ms/op`);
  return mean;
}

async function timeAsync(name, fn) {
  for (let i = 0; i < WARMUP; i++) await fn();
  const start = performance.now();
  for (let i = 0; i < ITERATIONS; i++) await fn();
  const elapsed = performance.now() - start;
  const mean = elapsed / ITERATIONS;
  console.log(`${name.padEnd(55)} ${mean.toFixed(4).padStart(8)} ms/op`);
  return mean;
}

console.log(`\n--- Breakdown: plugin path (${ITERATIONS} iterations) ---\n`);

// Pre-compute a HAST buffer for reader-based benchmarks
const preHastHandle = createHastHandle(MARKDOWN);
const hastBuf = serializeHandle(preHastHandle);
dropHandle(preHastHandle);

const t1 = time("1. createMdastHandle (Rust)", () => {
  const h = createMdastHandle(MARKDOWN);
  dropHandle(h);
});
const t2 = time("2. convertMdastToHastHandle (Rust)", () => {
  const mdast = createMdastHandle(MARKDOWN);
  const hast = convertMdastToHastHandle(mdast);
  dropHandle(hast);
});

// Measure just the visitor with a noop plugin
const t3 = await timeAsync("3. visitHast — noop plugin (JS walk + materialize)", async () => {
  const reader = new HastReader(hastBuf);
  const dataMap = new DataMap();
  await visitHast(reader, { element() {} }, dataMap);
});

const t4 = time("4. renderHandle (Rust)", () => {
  const hast = createHastHandle(MARKDOWN);
  renderHandle(hast);
  dropHandle(hast);
});

console.log(`\n--- Totals ---\n`);
console.log(`Rust-only (1+2+4):    ${(t1 + t2 + t4).toFixed(4)} ms`);
console.log(`JS visitor (3):       ${t3.toFixed(4)} ms`);
console.log(`Total with plugin:    ${(t1 + t2 + t3 + t4).toFixed(4)} ms`);
console.log(`Plugin overhead:      ${((t3 / (t1 + t2 + t4)) * 100).toFixed(0)}% of Rust time`);

// Now measure visitor internals
console.log(`\n--- Visitor internals ---\n`);

// Just constructing the reader
time("  HastReader construction", () => new HastReader(hastBuf));

// Reader + walk (no materialization)
const reader0 = new HastReader(hastBuf);
time("  Walk all nodes (getNodeType + getChildIds)", () => {
  const stack = [0];
  while (stack.length > 0) {
    const nodeId = stack.pop();
    reader0.getNodeType(nodeId);
    const childIds = reader0.getChildIds(nodeId);
    for (let i = childIds.length - 1; i >= 0; i--) {
      stack.push(childIds[i]);
    }
  }
});

// Reader + walk + materialize
import { materializeHastNode } from "../dist/hast/hast-materializer.js";
const reader1 = new HastReader(hastBuf);
const dm = new DataMap();
time("  Walk + materialize all nodes", () => {
  const stack = [0];
  while (stack.length > 0) {
    const nodeId = stack.pop();
    materializeHastNode(reader1, nodeId, dm);
    const childIds = reader1.getChildIds(nodeId);
    for (let i = childIds.length - 1; i >= 0; i--) {
      stack.push(childIds[i]);
    }
  }
});

console.log("");
