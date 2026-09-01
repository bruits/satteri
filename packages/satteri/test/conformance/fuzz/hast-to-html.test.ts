import fc from "fast-check";
import rehypeStringify from "rehype-stringify";
import { unified } from "unified";
import { describe, expect, test } from "vitest";
import { hastToHtml, htmlToHast } from "../../../src/index.js";
import type { HastNode } from "../../../src/index.js";
import { FC_OPTIONS, FUZZ_TIMEOUT_MS } from "./shared.js";

const reference = unified().use(rehypeStringify, {
  allowDangerousHtml: true,
  characterReferences: { useNamedReferences: true },
});

const referenceHtml = (tree: HastNode): string => reference.stringify(tree as never);

/** `hast-util-to-html` leaves `>`, `'` and `` ` `` bare where the renderer,
 *  following cmark, escapes them; both are valid HTML for the same text. */
const foldEscapes = (html: string): string =>
  html.replaceAll("&gt;", ">").replaceAll("&#x27;", "'").replaceAll("&#x60;", "`");

// Spread, not `split("")`: a lone surrogate has no UTF-8 form, so the native
// boundary replaces it and the comparison would fail on the generator's input.
const TEXT = fc.string({
  unit: fc.constantFrom(...[...`ab 0<>&"'\`\n\t\\{}émoji🎉`]),
  maxLength: 12,
});

const HTML_TAGS = ["div", "p", "span", "a", "li", "pre", "code", "button", "table"];
const VOID_TAGS = ["br", "img", "hr", "input", "meta"];
const RAW_TEXT_TAGS = ["script", "style"];
const SVG_TAGS = ["circle", "path", "text"];

// An empty or whitespace-only token survives one serialization but not the
// re-parse, so list values stay to tokens a document could carry.
const TOKEN = fc.string({
  unit: fc.constantFrom(..."abc012-".split("")),
  minLength: 1,
  maxLength: 6,
});

const ANY_SCHEMA_PROPERTY = fc.oneof(
  fc.tuple(fc.constant("id"), TEXT),
  fc.tuple(fc.constant("title"), TEXT),
  fc.tuple(fc.constant("className"), fc.array(TOKEN, { maxLength: 3 })),
  fc.tuple(fc.constant("disabled"), fc.boolean()),
  fc.tuple(fc.constant("tabIndex"), fc.integer({ min: -1, max: 9 })),
  fc.tuple(fc.constant("srcSet"), fc.array(TOKEN, { maxLength: 2 })),
  fc.tuple(fc.constant("strokeWidth"), fc.integer({ min: 0, max: 9 })),
  fc.tuple(fc.constant("data-x-y"), TEXT),
  fc.tuple(fc.constant("hidden"), fc.constant(false)),
);

// Comma-separated in the HTML schema only. The encoder keys the separator on
// the property name alone, so it would join these with commas inside `<svg>`
// too, where rehype switches schemas and joins with spaces.
const HTML_ONLY_PROPERTY = fc.oneof(
  fc.tuple(fc.constant("accept"), fc.array(TOKEN, { maxLength: 3 })),
  fc.tuple(fc.constant("coords"), fc.array(fc.integer({ min: 0, max: 99 }), { maxLength: 3 })),
  fc.tuple(fc.constant("exportParts"), fc.array(TOKEN, { maxLength: 2 })),
);

const toProperties = (entries: [string, unknown][]): Record<string, unknown> =>
  Object.fromEntries(entries) as Record<string, unknown>;

const properties = fc
  .array(fc.oneof(ANY_SCHEMA_PROPERTY, HTML_ONLY_PROPERTY), { maxLength: 4 })
  .map(toProperties);

const svgProperties = fc.array(ANY_SCHEMA_PROPERTY, { maxLength: 4 }).map(toProperties);

const leaf: fc.Arbitrary<HastNode> = fc.oneof(
  TEXT.map((value) => ({ type: "text", value }) as unknown as HastNode),
  TEXT.map((value) => ({ type: "comment", value }) as unknown as HastNode),
  fc
    .constantFrom("<b>x</b>", "<!-- c -->", "&amp;", "<p>", "")
    .map((value) => ({ type: "raw", value }) as unknown as HastNode),
  fc
    .tuple(fc.constantFrom(...VOID_TAGS), properties)
    .map(
      ([tagName, props]) =>
        ({ type: "element", tagName, properties: props, children: [] }) as unknown as HastNode,
    ),
);

// Only text children under <script>/<style>: an element there cannot come from
// parsing, and the renderer keeps raw-text escaping off for the whole subtree
// where `hast-util-to-html` looks at the text node's direct parent only.
const rawTextElement = fc
  .tuple(fc.constantFrom(...RAW_TEXT_TAGS), properties, fc.array(TEXT, { maxLength: 2 }))
  .map(
    ([tagName, props, values]) =>
      ({
        type: "element",
        tagName,
        properties: props,
        children: values.map((value) => ({ type: "text", value })),
      }) as unknown as HastNode,
  );

const element: fc.Arbitrary<HastNode> = fc.letrec<{ node: HastNode }>((tie) => ({
  node: fc.oneof(
    { depthSize: "small", withCrossShrink: true },
    leaf,
    rawTextElement,
    fc
      .tuple(fc.constantFrom(...HTML_TAGS), properties, fc.array(tie("node"), { maxLength: 3 }))
      .map(
        ([tagName, props, children]) =>
          ({ type: "element", tagName, properties: props, children }) as unknown as HastNode,
      ),
    fc
      .tuple(
        svgProperties,
        fc.array(fc.tuple(fc.constantFrom(...SVG_TAGS), svgProperties), { maxLength: 2 }),
      )
      .map(
        ([props, kids]) =>
          ({
            type: "element",
            tagName: "svg",
            properties: props,
            children: kids.map(([tagName, childProps]) => ({
              type: "element",
              tagName,
              properties: childProps,
              children: [],
            })),
          }) as unknown as HastNode,
      ),
  ),
})).node;

const tree = fc
  .array(element, { minLength: 1, maxLength: 4 })
  .map((children) => ({ type: "root", children }) as unknown as HastNode);

// Tags whose misnesting the parser repairs (`<a>` inside `<a>`, `<div>` inside
// `<p>`, a stray `<li>`) have no parse/serialize fixed point, so the round-trip
// grammar sticks to elements the parser nests as written.
const NESTABLE_TAGS = ["div", "span", "section"];

const parseableElement: fc.Arbitrary<HastNode> = fc.letrec<{ node: HastNode }>((tie) => ({
  node: fc.oneof(
    { depthSize: "small", withCrossShrink: true },
    TEXT.map((value) => ({ type: "text", value }) as unknown as HastNode),
    TEXT.map((value) => ({ type: "comment", value }) as unknown as HastNode),
    fc
      .tuple(fc.constantFrom("br", "img"), properties)
      .map(
        ([tagName, props]) =>
          ({ type: "element", tagName, properties: props, children: [] }) as unknown as HastNode,
      ),
    rawTextElement,
    fc
      .tuple(fc.constantFrom(...NESTABLE_TAGS), properties, fc.array(tie("node"), { maxLength: 3 }))
      .map(
        ([tagName, props, children]) =>
          ({ type: "element", tagName, properties: props, children }) as unknown as HastNode,
      ),
  ),
})).node;

const parseableTree = fc
  .array(parseableElement, { minLength: 1, maxLength: 4 })
  .map((children) => ({ type: "root", children }) as unknown as HastNode);

describe("fuzz: hastToHtml conformance", () => {
  test(
    "serializes generated trees like hast-util-to-html",
    () => {
      fc.assert(
        fc.property(tree, (node) => {
          expect(foldEscapes(hastToHtml(node))).toBe(foldEscapes(referenceHtml(node)));
        }),
        FC_OPTIONS,
      );
    },
    FUZZ_TIMEOUT_MS,
  );

  test(
    "re-parses its own output to the same HTML",
    () => {
      fc.assert(
        fc.property(parseableTree, (node) => {
          const once = hastToHtml(node);
          const reparsed = hastToHtml(htmlToHast(once, { fragment: true }));
          expect(hastToHtml(htmlToHast(reparsed, { fragment: true }))).toBe(reparsed);
        }),
        FC_OPTIONS,
      );
    },
    FUZZ_TIMEOUT_MS,
  );
});
