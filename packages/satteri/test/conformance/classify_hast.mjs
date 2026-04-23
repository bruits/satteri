import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import { toHast } from "mdast-util-to-hast";
import { pathToFileURL } from "node:url";
import { mdxToMdast, mdxToHast } from "../../dist/index.js";

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
const MDX_PASS_THROUGH = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

const emptyDirectiveHandler = () => undefined;
const REF_TO_HAST_OPTIONS = {
  allowDangerousHtml: true,
  passThrough: MDX_PASS_THROUGH,
  handlers: {
    containerDirective: emptyDirectiveHandler,
    leafDirective: emptyDirectiveHandler,
    textDirective: emptyDirectiveHandler,
  },
};

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

function normalizeAlignToStyle(node) {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(normalizeAlignToStyle);
  const out = { ...node };
  delete out.data;
  if (out.properties && typeof out.properties === "object") {
    const props = { ...out.properties };
    if ("align" in props && typeof props.align === "string") {
      props.style = `text-align: ${props.align}`;
      delete props.align;
    }
    out.properties = props;
  }
  if (Array.isArray(out.children)) out.children = out.children.map(normalizeAlignToStyle);
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

function classifyHast(ref, sat) {
  if (!ref || !sat) return "other/missing";
  const rt = ref.type ?? "?";
  const st = sat.type ?? "?";

  // Different type entirely
  if (rt !== st) return `TYPE: ref=${rt} sat=${st}`;

  // Both are element — check tagName/properties
  if (rt === "element") {
    if (ref.tagName !== sat.tagName) return `element.tagName: ${ref.tagName} vs ${sat.tagName}`;
    const refP = ref.properties ?? {};
    const satP = sat.properties ?? {};
    const refKeys = Object.keys(refP).sort().join(",");
    const satKeys = Object.keys(satP).sort().join(",");
    if (refKeys !== satKeys) return `element[${ref.tagName}].properties keys: ${refKeys} vs ${satKeys}`;
    for (const k of Object.keys(refP)) {
      if (JSON.stringify(refP[k]) !== JSON.stringify(satP[k])) {
        return `element[${ref.tagName}].properties.${k}`;
      }
    }
    const rc = ref.children?.length ?? 0;
    const sc = sat.children?.length ?? 0;
    if (rc !== sc) return `element[${ref.tagName}].children length ${rc} vs ${sc}`;
  }

  if (rt === "text" && st === "text") {
    return `text: "${JSON.stringify(ref.value).slice(0, 30)}" vs "${JSON.stringify(sat.value).slice(0, 30)}"`;
  }

  if (rt === "root") {
    const rc = ref.children?.length ?? 0;
    const sc = sat.children?.length ?? 0;
    if (rc !== sc) return `root.children length ${rc} vs ${sc}`;
  }

  return `other: ${rt}`;
}

const files = walk(DOCS).sort();
const buckets = new Map();
const examples = new Map();
let mdastOkHastFail = 0;
let bothFail = 0;

for (const f of files) {
  const src = readFileSync(f, "utf8");
  let refMdast, refHast, satMdast, satHast;
  try {
    refMdast = strip(refParser.runSync(refParser.parse(src)));
    refHast = strip(
      normalizeAlignToStyle(
        toHast(refParser.runSync(refParser.parse(src)), REF_TO_HAST_OPTIONS),
      ),
    );
    satMdast = strip(mdxToMdast(src, { features: FEATURES }));
    satHast = strip(mdxToHast(src, { features: FEATURES }));
  } catch {
    continue;
  }

  const mdastEq = eq(refMdast, satMdast);
  const hastEq = eq(refHast, satHast);

  if (hastEq) continue; // HAST matches — not our concern
  if (!mdastEq) {
    bothFail++;
    continue;
  }
  mdastOkHastFail++;

  const mini = minimal(refHast, satHast);
  if (!mini) continue;
  const key = classifyHast(mini.ref, mini.sat);
  buckets.set(key, (buckets.get(key) ?? 0) + 1);
  if (!examples.has(key)) {
    examples.set(key, { file: relative(DOCS, f), ref: mini.ref, sat: mini.sat });
  }
}

console.log(`\n=== HAST-only divergences (MDAST matches, HAST fails) ===`);
console.log(`Total:         ${mdastOkHastFail} files`);
console.log(`Both fail too: ${bothFail} files`);
console.log();
const sorted = [...buckets.entries()].sort((a, b) => b[1] - a[1]);
for (const [key, count] of sorted) {
  console.log(`${count.toString().padStart(4)}× ${key}`);
  const ex = examples.get(key);
  if (ex) {
    console.log(`       e.g. ${ex.file}`);
    console.log(`          REF: ${JSON.stringify(ex.ref).slice(0, 180)}`);
    console.log(`          SAT: ${JSON.stringify(ex.sat).slice(0, 180)}`);
  }
  console.log();
}
