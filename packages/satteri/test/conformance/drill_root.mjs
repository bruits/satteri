import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";
import remarkGfm from "remark-gfm";
import { pathToFileURL } from "node:url";
import { mdxToMdast } from "../../dist/index.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href,
);

const refParser = remark()
  .use(remarkMdx)
  .use(remarkGfm)
  .use(remarkFrontmatter, ["yaml", "toml"])
  .use(remarkDirective)
  .use(remarkMarkAndUnravel);

const FEATURES = { frontmatter: true, directive: true };
const DOCS = "/home/erika/Projects/docs";

function eq(a, b) {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (typeof a !== "object" || a === null || b === null) return a === b;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
    return true;
  }
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  for (const k of ka) if (!eq(a[k], b[k])) return false;
  return true;
}

function strip(node) {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(strip);
  const out = {};
  for (const [k, v] of Object.entries(node)) {
    if (k === "position" || k === "data") continue;
    if (Array.isArray(v)) out[k] = v.map(strip);
    else if (typeof v === "object" && v !== null) out[k] = strip(v);
    else out[k] = v;
  }
  return out;
}

function shapeOf(n) {
  if (!n || typeof n !== "object") return typeof n;
  const type = n.type ?? "?";
  const name = n.name ? `(${n.name})` : "";
  return `${type}${name}`;
}

// When children lengths differ, find the first index where the shape diverges.
function firstMismatchShape(refChildren, satChildren) {
  const max = Math.max(refChildren.length, satChildren.length);
  for (let i = 0; i < max; i++) {
    const r = refChildren[i];
    const s = satChildren[i];
    if (shapeOf(r) !== shapeOf(s)) {
      return { i, ref: shapeOf(r), sat: shapeOf(s) };
    }
  }
  return null;
}

// For cases where children counts match but deep content differs and shapes
// differ at some point — find the smallest node where shapes disagree.
function findShapeDiff(ref, sat) {
  if (eq(ref, sat)) return null;
  if (typeof ref !== "object" || typeof sat !== "object") return { ref, sat };
  if (Array.isArray(ref) && Array.isArray(sat)) {
    if (ref.length !== sat.length) return { arrayDiff: true, ref: ref.length, sat: sat.length };
    for (let i = 0; i < ref.length; i++) {
      const f = findShapeDiff(ref[i], sat[i]);
      if (f) return f;
    }
    return null;
  }
  if (ref.type && ref.type !== sat.type) {
    return { typeDiff: true, ref: shapeOf(ref), sat: shapeOf(sat) };
  }
  if (Array.isArray(ref.children) && Array.isArray(sat.children)) {
    if (ref.children.length !== sat.children.length) {
      const where = firstMismatchShape(ref.children, sat.children);
      return {
        parent: shapeOf(ref),
        childCount: `${ref.children.length} vs ${sat.children.length}`,
        firstDiff: where,
      };
    }
    for (let i = 0; i < ref.children.length; i++) {
      const f = findShapeDiff(ref.children[i], sat.children[i]);
      if (f) return f;
    }
  }
  // Same type, same children length, but something else differs (e.g., value,
  // url, title). Report a shape-level summary.
  const keys = new Set([...Object.keys(ref), ...Object.keys(sat)]);
  for (const k of keys) {
    if (k === "children" || k === "type") continue;
    if (JSON.stringify(ref[k]) !== JSON.stringify(sat[k])) {
      return { parent: shapeOf(ref), field: k, ref: ref[k], sat: sat[k] };
    }
  }
  return null;
}

function walk(dir, exclude = new Set(["node_modules", "dist", ".git", ".astro"])) {
  const out = [];
  for (const e of readdirSync(dir)) {
    if (exclude.has(e)) continue;
    const full = join(dir, e);
    const st = statSync(full);
    if (st.isDirectory()) out.push(...walk(full, exclude));
    else if (st.isFile() && full.endsWith(".mdx")) out.push(full);
  }
  return out;
}

const files = walk(DOCS).sort();
const buckets = new Map();
const examples = new Map();

for (const f of files) {
  const src = readFileSync(f, "utf8");
  let ref, sat;
  try {
    ref = strip(refParser.runSync(refParser.parse(src)));
    sat = strip(mdxToMdast(src, { features: FEATURES }));
  } catch {
    continue;
  }
  if (eq(ref, sat)) continue;

  const diff = findShapeDiff(ref, sat);
  if (!diff) continue;
  let key;
  if (diff.firstDiff) {
    key = `${diff.parent} child-count ${diff.childCount}  first-diff-shape=${diff.firstDiff.ref}→${diff.firstDiff.sat}`;
  } else if (diff.typeDiff) {
    key = `TYPE ${diff.ref}→${diff.sat}`;
  } else if (diff.arrayDiff) {
    key = `array length ${diff.ref} vs ${diff.sat}`;
  } else if (diff.field) {
    key = `${diff.parent}.${diff.field}`;
  } else {
    key = "unknown";
  }
  buckets.set(key, (buckets.get(key) ?? 0) + 1);
  if (!examples.has(key)) {
    examples.set(key, { file: relative(DOCS, f), diff });
  }
}

console.log(`\n=== Remaining MDAST divergences (deep classification) ===\n`);
const sorted = [...buckets.entries()].sort((a, b) => b[1] - a[1]);
for (const [key, count] of sorted) {
  const ex = examples.get(key);
  console.log(`${count.toString().padStart(4)}× ${key}`);
  if (ex) {
    console.log(`       e.g. ${ex.file}`);
    const refS = JSON.stringify(ex.diff.ref ?? ex.diff).slice(0, 100);
    const satS = JSON.stringify(ex.diff.sat ?? ex.diff).slice(0, 100);
    if (refS !== satS) {
      console.log(`          REF: ${refS}`);
      console.log(`          SAT: ${satS}`);
    }
  }
  console.log();
}

console.log(`Total divergent files: ${[...buckets.values()].reduce((a, b) => a + b, 0)}`);
