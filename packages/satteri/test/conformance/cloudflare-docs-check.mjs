import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { performance } from "node:perf_hooks";
import { remark } from "remark";
import remarkMdx from "remark-mdx";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";
import remarkGfm from "remark-gfm";
import { toHast } from "mdast-util-to-hast";
import { pathToFileURL } from "node:url";
import { mdxToMdast, mdxToHast } from "../../dist/index.js";

const { remarkMarkAndUnravel } = await import(
  pathToFileURL("node_modules/@mdx-js/mdx/lib/plugin/remark-mark-and-unravel.js").href
);

const DOCS_ROOT = "/home/erika/Projects/cloudflare-docs";
const FEATURES = { frontmatter: true, directive: true, math: false };

const MDX_PASS_THROUGH = [
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
];

// Satteri drops directives during mdast→hast conversion; the reference side
// mirrors this with empty handlers.
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

const referenceParser = remark()
  .use(remarkMdx)
  .use(remarkGfm)
  .use(remarkFrontmatter, ["yaml", "toml"])
  .use(remarkDirective)
  .use(remarkMarkAndUnravel);

function stripPositionsAndEstree(node) {
  if (typeof node !== "object" || node === null) return node;
  if (Array.isArray(node)) return node.map(stripPositionsAndEstree);
  const out = {};
  for (const [k, v] of Object.entries(node)) {
    if (k === "position" || k === "data") continue;
    if (k === "properties" && typeof v === "object" && v !== null) {
      // Satteri emits modern `style="text-align: <x>"` for HAST table cells
      // where `mdast-util-to-hast` still emits the deprecated `align` prop;
      // fold that down on the reference side so the comparison only catches
      // structural divergence.
      const props = { ...v };
      if ("align" in props && typeof props.align === "string") {
        props.style = `text-align: ${props.align}`;
        delete props.align;
      }
      out[k] = props;
      continue;
    }
    if (Array.isArray(v)) out[k] = v.map(stripPositionsAndEstree);
    else if (typeof v === "object" && v !== null) out[k] = stripPositionsAndEstree(v);
    else out[k] = v;
  }
  return out;
}

function referenceMdxMdast(input) {
  const parsed = referenceParser.parse(input);
  const transformed = referenceParser.runSync(parsed);
  return stripPositionsAndEstree(transformed);
}

function referenceMdxHast(input) {
  const parsed = referenceParser.parse(input);
  const transformed = referenceParser.runSync(parsed);
  return stripPositionsAndEstree(toHast(transformed, REF_TO_HAST_OPTIONS));
}

function satteriMdxMdast(input) {
  return stripPositionsAndEstree(mdxToMdast(input, { features: FEATURES }));
}

function satteriMdxHast(input) {
  return stripPositionsAndEstree(mdxToHast(input, { features: FEATURES }));
}

function walk(dir, exclude = new Set(["node_modules", "dist", ".git", ".astro"])) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    if (exclude.has(entry)) continue;
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) out.push(...walk(full, exclude));
    else if (st.isFile() && full.endsWith(".mdx")) out.push(full);
  }
  return out;
}

function firstDiff(expected, actual, path = "$") {
  if (typeof expected !== typeof actual) {
    return `${path}: type ${typeof expected} vs ${typeof actual}`;
  }
  if (typeof expected !== "object" || expected === null || actual === null) {
    if (expected !== actual) {
      const e = JSON.stringify(expected);
      const a = JSON.stringify(actual);
      return `${path}: ${e?.slice(0, 80)} vs ${a?.slice(0, 80)}`;
    }
    return null;
  }
  if (Array.isArray(expected) !== Array.isArray(actual)) {
    return `${path}: array/object mismatch`;
  }
  if (Array.isArray(expected)) {
    if (expected.length !== actual.length) {
      return `${path}: array length ${expected.length} vs ${actual.length}`;
    }
    for (let i = 0; i < expected.length; i++) {
      const d = firstDiff(expected[i], actual[i], `${path}[${i}]`);
      if (d) return d;
    }
    return null;
  }
  const keys = new Set([...Object.keys(expected), ...Object.keys(actual)]);
  for (const k of keys) {
    if (!(k in expected)) return `${path}.${k}: missing in expected`;
    if (!(k in actual)) return `${path}.${k}: missing in actual`;
    const d = firstDiff(expected[k], actual[k], `${path}.${k}`);
    if (d) return d;
  }
  return null;
}

function classifyDiff(diff) {
  if (!diff) return "ok";
  return diff
    .replace(/\[\d+\]/g, "[N]")
    .replace(/:\s.*$/, "")
    .slice(0, 120);
}

const files = walk(DOCS_ROOT).sort();
console.log(`Found ${files.length} .mdx files under ${DOCS_ROOT}`);

const totals = { checked: 0, mdastOk: 0, hastOk: 0, mdastFail: 0, hastFail: 0, parseErr: 0 };
const bucketsMdast = new Map();
const bucketsHast = new Map();
const parseErrors = [];

const start = performance.now();
for (const file of files) {
  totals.checked++;
  const src = readFileSync(file, "utf8");
  const rel = relative(DOCS_ROOT, file);

  let refMdast, refHast, satMdast, satHast;
  try {
    refMdast = referenceMdxMdast(src);
    refHast = referenceMdxHast(src);
  } catch (e) {
    parseErrors.push({ rel, side: "reference", error: String(e).slice(0, 200) });
    totals.parseErr++;
    continue;
  }
  try {
    satMdast = satteriMdxMdast(src);
    satHast = satteriMdxHast(src);
  } catch (e) {
    parseErrors.push({ rel, side: "satteri", error: String(e).slice(0, 200) });
    totals.parseErr++;
    continue;
  }

  const mdastDiff = firstDiff(refMdast, satMdast);
  if (mdastDiff) {
    totals.mdastFail++;
    const key = classifyDiff(mdastDiff);
    const list = bucketsMdast.get(key) ?? [];
    list.push({ rel, diff: mdastDiff });
    bucketsMdast.set(key, list);
  } else {
    totals.mdastOk++;
  }

  const hastDiff = firstDiff(refHast, satHast);
  if (hastDiff) {
    totals.hastFail++;
    const key = classifyDiff(hastDiff);
    const list = bucketsHast.get(key) ?? [];
    list.push({ rel, diff: hastDiff });
    bucketsHast.set(key, list);
  } else {
    totals.hastOk++;
  }

  if (totals.checked % 250 === 0) {
    process.stdout.write(
      `  ${totals.checked}/${files.length}  mdast-ok=${totals.mdastOk}  hast-ok=${totals.hastOk}  parseErr=${totals.parseErr}\n`,
    );
  }
}
const elapsed = ((performance.now() - start) / 1000).toFixed(1);

console.log("\n=== Summary ===");
console.log(`Files:        ${totals.checked}  (elapsed ${elapsed}s)`);
console.log(
  `MDAST:        ${totals.mdastOk} ok / ${totals.mdastFail} fail   (${((totals.mdastOk / (totals.mdastOk + totals.mdastFail)) * 100).toFixed(1)}%)`,
);
console.log(
  `HAST:         ${totals.hastOk} ok / ${totals.hastFail} fail   (${((totals.hastOk / (totals.hastOk + totals.hastFail)) * 100).toFixed(1)}%)`,
);
console.log(`Parse errors: ${totals.parseErr}`);

function report(title, buckets) {
  if (buckets.size === 0) return "";
  const sorted = [...buckets.entries()].sort((a, b) => b[1].length - a[1].length);
  const lines = [`\n## ${title} — ${sorted.length} unique pattern(s)`, ""];
  for (const [key, list] of sorted) {
    lines.push(`### ${list.length}× \`${key}\``);
    for (const { rel, diff } of list.slice(0, 3)) {
      lines.push(`- ${rel}`);
      lines.push(`  - ${diff}`);
    }
    if (list.length > 3) lines.push(`  … and ${list.length - 3} more`);
    lines.push("");
  }
  return lines.join("\n");
}

const outPath = new URL("./CLOUDFLARE-DOCS-CHECK.md", import.meta.url);
const reportBody = [
  "# Cloudflare docs conformance check",
  "",
  `- Root: \`${DOCS_ROOT}\``,
  `- Features: MDX + frontmatter + directive`,
  `- Files: ${totals.checked}`,
  `- MDAST: ${totals.mdastOk} ok / ${totals.mdastFail} fail`,
  `- HAST:  ${totals.hastOk} ok / ${totals.hastFail} fail`,
  `- Parse errors: ${totals.parseErr}`,
  "",
  report("MDAST mismatches", bucketsMdast),
  report("HAST mismatches", bucketsHast),
  parseErrors.length
    ? `\n## Parse errors\n\n${parseErrors
        .slice(0, 20)
        .map((e) => `- [${e.side}] ${e.rel}\n  - ${e.error}`)
        .join("\n")}`
    : "",
].join("\n");

writeFileSync(outPath, reportBody);
console.log(`\nReport written to ${outPath.pathname}`);

if (totals.mdastFail > 0 || totals.hastFail > 0) {
  console.log("\nTop MDAST buckets:");
  const topMdast = [...bucketsMdast.entries()].sort((a, b) => b[1].length - a[1].length).slice(0, 5);
  for (const [k, list] of topMdast) {
    console.log(`  ${list.length}×  ${k}`);
  }
  console.log("Top HAST buckets:");
  const topHast = [...bucketsHast.entries()].sort((a, b) => b[1].length - a[1].length).slice(0, 5);
  for (const [k, list] of topHast) {
    console.log(`  ${list.length}×  ${k}`);
  }
}
