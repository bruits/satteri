import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import { markdownToHast } from "../src/index.js";
import type { HastNode } from "../src/hast/hast-materializer.js";

/**
 * Conformance suite for the `rawHtml` feature (the `rehype-raw` equivalent).
 *
 * Each input is run through both:
 *  - Sätteri: `markdownToHast(md, { features: { rawHtml: true } })`
 *  - unified: remark-parse → remark-rehype (allowDangerousHtml) → rehype-raw
 *
 * and compared two ways — serialized HTML (via rehype-stringify) and the hast
 * tree itself (structure + normalized properties, positions stripped). The
 * inputs are chosen so Sätteri's baseline Markdown→hast already matches
 * remark-rehype's; the feature under test is the raw-HTML reparsing.
 */

const reference = unified()
  .use(remarkParse)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeRaw)
  .use(rehypeStringify);

const referenceTree = (md: string): HastNode =>
  reference.runSync(reference.parse(md)) as unknown as HastNode;

const stringify = (tree: HastNode): string =>
  unified()
    .use(rehypeStringify)
    .stringify(tree as never);

/** Keep only structural fields so trees compare regardless of positions/internals. */
function clean(node: HastNode): unknown {
  const out: Record<string, unknown> = { type: node.type };
  if (node.type === "element") {
    out.tagName = node.tagName;
    out.properties = { ...node.properties };
  }
  if ((node.type === "text" || node.type === "comment" || node.type === "raw") && "value" in node) {
    out.value = node.value;
  }
  if ("children" in node && node.children) {
    out.children = (node.children as HastNode[]).map(clean);
  }
  return out;
}

const cases: Array<{ name: string; md: string }> = [
  { name: "block element wrapping markdown", md: `<div class="note">\n\ntext **bold**\n\n</div>` },
  { name: "inline html", md: `A <span id="s">x</span> and text` },
  {
    name: "normalized attributes",
    md: `<img src="a.png" width="10" disabled class="a b">`,
  },
  { name: "data + aria attributes", md: `<div data-foo-bar="1" aria-label="x"></div>` },
  { name: "comment", md: `<div><!--note--></div>` },
  { name: "tag split across raw blocks", md: `<div class="wrap">\n\n# Heading\n\n</div>` },
  { name: "misnested tags (adoption agency)", md: `<b>1<p>2</b>3</p>` },
  { name: "table with implied tbody", md: `<table><tr><td>y</td></tr></table>` },
  { name: "void + boolean attrs", md: `<input type="checkbox" checked>` },
  { name: "heading then raw", md: `# Hi\n\n<p class="x">para</p>` },
  // Boolean coercion: true only for empty or name-matching values.
  { name: "boolean with false value", md: `<input disabled="false">` },
  { name: "boolean with zero value", md: `<input checked="0">` },
  { name: "boolean repeating its name", md: `<option selected="selected">x</option>` },
  { name: "boolean repeating its name uppercase", md: `<input disabled="DISABLED">` },
  { name: "overloaded boolean repeating its name", md: `<a download="download">x</a>` },
  { name: "overloaded boolean with empty value", md: `<a download="">e</a>` },
  { name: "overloaded boolean with a filename", md: `<a download="f.txt">x</a>` },
  { name: "hidden repeating its name", md: `<div hidden="hidden">y</div>` },
  { name: "itemscope", md: `<div itemscope>x</div>` },
  {
    name: "boolean-ish values stay strings",
    md: `<div contenteditable="true" draggable="false" spellcheck aria-hidden="true">x</div>`,
  },
  // Number coercion follows JavaScript Number() semantics.
  { name: "number simple", md: `<img width="3">` },
  { name: "number float", md: `<img width="3.5">` },
  { name: "number exponent", md: `<img width="1e3">` },
  { name: "number leading dot", md: `<img width=".5">` },
  { name: "number signed", md: `<img width="+5"><img height="-0">` },
  { name: "number surrounding whitespace", md: `<img width=" 3 ">` },
  { name: "number hex", md: `<img width="0x10">` },
  { name: "number binary", md: `<img width="0b101">` },
  { name: "number octal", md: `<img width="0o17">` },
  { name: "number Infinity", md: `<img width="Infinity"><img height="-Infinity">` },
  { name: "number rejects Rust-only spellings", md: `<img width="inf"><img height="NaN">` },
  { name: "number rejects units and separators", md: `<img width="12px"><img height="1_000">` },
  { name: "number empty stays string", md: `<img width="">` },
  { name: "negative tabindex", md: `<div tabindex="-1">x</div>` },
  { name: "aria number", md: `<div aria-valuenow="5">x</div>` },
  // List coercion.
  { name: "space-separated with mixed whitespace", md: `<div class="a\tb\nc  d ">x</div>` },
  { name: "space-separated empty", md: `<div class="">x</div>` },
  { name: "comma-separated with spaces", md: `<input accept=".png, .jpg">` },
  { name: "comma-separated trailing comma", md: `<input accept=".png,">` },
  { name: "comma-separated interior empty item", md: `<input accept=".png,,.jpg">` },
  { name: "coords items become numbers", md: `<area coords="1,2, 3">` },
  // data-* and unknown attributes.
  { name: "data attribute with digit segment", md: `<div data-x-1="y">z</div>` },
  { name: "data attribute multi segment", md: `<div data-a-b-c="1">x</div>` },
  { name: "bare data attribute", md: `<div data-foo>x</div>` },
  { name: "unknown attributes pass through", md: `<div foo="bar" my-attr="1">x</div>` },
  // SVG schema.
  {
    name: "svg attributes keep casing",
    md: `<svg viewBox="0 0 10 10"><path fill-rule="evenodd" stroke-width="2"/></svg>`,
  },
  { name: "svg dasharray", md: `<svg><path stroke-dasharray="1, 2 3"/></svg>` },
  {
    name: "svg foreignObject",
    md: `<svg><foreignObject><div class="a">x</div></foreignObject></svg>`,
  },
  // Structure: context-sensitive elements outside their usual homes survive
  // (the fragment parses in a template context).
  { name: "bare td", md: `<td headers=" h1  h2 ">x</td>` },
  { name: "bare tr", md: `<tr><td>a</td><td>b</td></tr>` },
  { name: "bare thead", md: `<thead><tr><th>h</th></tr></thead>` },
  { name: "bare caption", md: `<caption>c</caption>` },
  { name: "bare colgroup", md: `<colgroup><col span="2"></colgroup>` },
  { name: "bare options", md: `<option>a</option><option>b</option>` },
  { name: "bare li", md: `<li>a</li>` },
  { name: "bare dt dd", md: `<dt>t</dt><dd>d</dd>` },
  {
    name: "stray content foster-parented out of table",
    md: `<table><b>x</b><tr><td>y</td></tr></table>`,
  },
  { name: "unclosed paragraphs", md: `<p>a<p>b` },
  { name: "select with option", md: `<select><option>a</option></select>` },
  { name: "image alias becomes img", md: `<image src="a.png">` },
  { name: "meta then content", md: `<meta charset="utf-8"><p>x</p>` },
  { name: "body tag content", md: `<body class="b"><p>x</p></body>` },
  { name: "html tag content", md: `<html lang="en"><p>x</p></html>` },
  { name: "frameset", md: `<frameset><frame></frameset>` },
  { name: "nested forms", md: `<form><form><input></form></form>` },
  { name: "inline svg in paragraph", md: `<p><svg><circle r="1"/></svg>after</p>` },
  { name: "mathml", md: `<math><mi>x</mi></math>` },
];

describe("rawHtml conformance vs rehype-raw", () => {
  for (const { name, md } of cases) {
    test(`serialized HTML matches: ${name}`, () => {
      const ours = markdownToHast(md, { features: { rawHtml: true } });
      expect(stringify(ours)).toBe(reference.stringify(referenceTree(md)));
    });

    test(`hast tree matches: ${name}`, () => {
      const ours = markdownToHast(md, { features: { rawHtml: true } });
      expect(clean(ours)).toEqual(clean(referenceTree(md)));
    });
  }
});
