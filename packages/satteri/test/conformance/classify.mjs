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

function minimal(ref, sat) {
  if (eq(ref, sat)) return null;
  if (typeof ref !== "object" || typeof sat !== "object" || ref === null || sat === null) {
    return { ref, sat };
  }
  if (Array.isArray(ref) && Array.isArray(sat) && ref.length === sat.length) {
    for (let i = 0; i < ref.length; i++) {
      const f = minimal(ref[i], sat[i]);
      if (f) return f;
    }
  }
  if (
    !Array.isArray(ref) &&
    !Array.isArray(sat) &&
    ref.type &&
    ref.type === sat.type &&
    Array.isArray(ref.children) &&
    Array.isArray(sat.children) &&
    ref.children.length === sat.children.length
  ) {
    for (let i = 0; i < ref.children.length; i++) {
      const f = minimal(ref.children[i], sat.children[i]);
      if (f) return f;
    }
  }
  return { ref, sat };
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

function classify(ref, sat) {
  if (!ref || !sat) return "other/missing";
  const rt = ref.type ?? "?";
  const st = sat.type ?? "?";

  // Bug: <summary> treated as text element
  if (rt === "mdxJsxFlowElement" && ref.name === "summary" && st === "paragraph") {
    const firstChild = sat.children?.[0];
    if (firstChild?.type === "mdxJsxTextElement" && firstChild.name === "summary") {
      return "A: <summary> parsed as text element (paragraph-wrapped mdxJsxTextElement)";
    }
  }

  // Bug: other mdxJsxFlowElement wrapped in paragraph
  if (rt === "mdxJsxFlowElement" && st === "paragraph") {
    return `A': <${ref.name}> parsed as text element inside paragraph`;
  }

  // Bug: indented code block value preserving extra leading whitespace
  if (rt === "code" && st === "code" && ref.value !== sat.value) {
    if ((sat.value ?? "").startsWith(" ") && !(ref.value ?? "").startsWith(" ")) {
      return "B: code value has extra leading whitespace";
    }
    if (/\n /.test(sat.value ?? "") && !/\n /.test(ref.value ?? "")) {
      return "B: code value has extra leading whitespace";
    }
  }

  // Bug: meta string trailing whitespace stripped
  if (
    rt === "code" &&
    st === "code" &&
    ref.meta !== sat.meta &&
    typeof ref.meta === "string" &&
    ref.meta.trim() === sat.meta?.trim()
  ) {
    return "C: code meta trailing whitespace stripped";
  }

  // Bug: mdxjsEsm trailing whitespace
  if (rt === "mdxjsEsm" && st === "mdxjsEsm" && ref.value !== sat.value) {
    if (ref.value?.trim() === sat.value?.trim()) {
      return "D: mdxjsEsm trailing whitespace stripped";
    }
  }

  // Bug: GFM autolink literal not recognized
  if (
    rt === "text" &&
    st === "text" &&
    typeof sat.value === "string" &&
    typeof ref.value === "string" &&
    sat.value.includes("http") &&
    !ref.value.includes("http")
  ) {
    return "E: GFM autolink literal not extracted (bare URL left in text)";
  }

  // Bug: containerDirective absorbed into list
  if (rt === "root" || rt === "list") {
    // Root length diff or list spread/child-count diff with directive nearby
    const refLen = ref.children?.length ?? 0;
    const satLen = sat.children?.length ?? 0;
    if (refLen !== satLen) {
      const satLast = sat.children?.[satLen - 1];
      if (satLast?.type === "list") {
        const inner = satLast.children?.[satLast.children.length - 1];
        if (inner?.type === "containerDirective") {
          return "F: containerDirective absorbed into preceding list";
        }
        // Otherwise check: list.spread=true in sat but false in ref
        if (sat.type === "list" && sat.spread === true && ref.spread === false) {
          return "F': list spread flipped (likely due to following directive absorb)";
        }
      }
    }
  }

  // Bug: directive name truncation (unicode)
  if (
    (rt === "textDirective" || rt === "leafDirective" || rt === "containerDirective") &&
    (st === "textDirective" || st === "leafDirective" || st === "containerDirective") &&
    ref.name !== sat.name
  ) {
    return "G: directive name unicode truncation";
  }

  // Fallback
  return `other: ref=${rt} sat=${st}`;
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

  const mini = minimal(ref, sat);
  if (!mini) continue;

  const key = classify(mini.ref, mini.sat);
  buckets.set(key, (buckets.get(key) ?? 0) + 1);
  if (!examples.has(key)) {
    examples.set(key, { file: relative(DOCS, f), ref: mini.ref, sat: mini.sat });
  }
}

console.log("\n=== Root-cause classification of MDAST divergences ===\n");
const sorted = [...buckets.entries()].sort((a, b) => b[1] - a[1]);
for (const [key, count] of sorted) {
  console.log(`${count.toString().padStart(4)}× ${key}`);
  const ex = examples.get(key);
  if (ex) {
    console.log(`       e.g. ${ex.file}`);
    const refS = JSON.stringify(ex.ref).slice(0, 120);
    const satS = JSON.stringify(ex.sat).slice(0, 120);
    console.log(`          REF: ${refS}`);
    console.log(`          SAT: ${satS}`);
  }
  console.log();
}

const total = sorted.reduce((a, [, v]) => a + v, 0);
console.log(`Total divergent files: ${total}`);
