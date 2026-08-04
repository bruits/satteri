import { compile as mdxCompile, evaluate as mdxEvaluate } from "@mdx-js/mdx";
import type {
  CompileOptions as MdxCompileOptions,
  EvaluateOptions as MdxEvaluateOptions,
} from "@mdx-js/mdx";
import {
  evaluate as satteriEvaluate,
  defineHastPlugin,
  markdownToJs,
  markdownToMdast,
  markdownToHast,
  markdownToHtml,
  mdxToJs,
} from "../../src/index.js";
import type { Features, EvaluateOptions, MarkdownToJsOptions, HastNode } from "../../src/index.js";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import * as runtime from "react/jsx-runtime";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";

import remarkRehype from "remark-rehype";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import type { Nodes } from "hast";
import { expect } from "vitest";

const mdxRuntime = runtime as unknown as Pick<MdxEvaluateOptions, "Fragment" | "jsx" | "jsxs">;
const satteriRuntime = runtime as unknown as Pick<EvaluateOptions, "Fragment" | "jsx" | "jsxs">;

// Satteri's Rust mdast→hast converter can't see JS-level directive handlers,
// so by default it emits nothing for directive nodes. Match that on the
// reference side with empty `toHast` handlers; users who want to render
// directives are expected to plug in their own handler on both pipelines.
const emptyHandler = () => undefined;
export const REF_REHYPE_OPTIONS = {
  allowDangerousHtml: true,
  handlers: {
    containerDirective: emptyHandler,
    leafDirective: emptyHandler,
    textDirective: emptyHandler,
  },
} as const;

// Default reference is plain remark + GFM. We intentionally do NOT enable
// frontmatter or math here — remark-frontmatter has a quirk where enabling
// it changes how `---` interacts with surrounding content even when no yaml
// actually matches, which would make fuzz comparisons unstable.
//
// Satteri's `markdownToMdast(md)` default turns frontmatter/math on, so the
// plain helpers below pass `features: BASE_FEATURES` to disable them when
// comparing with this reference. Tests that specifically want frontmatter
// or math use `assertExt*` which build feature-matched processors.
const mdastProcessor = unified().use(remarkParse).use(remarkGfm);
const hastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkRehype, REF_REHYPE_OPTIONS);
const htmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkRehype, REF_REHYPE_OPTIONS)
  .use(rehypeStringify, { allowDangerousHtml: true });

const BASE_FEATURES: Features = { frontmatter: false, math: false };

export type ExtensionSet = "math" | "frontmatter" | "directive";

interface TestProcessor {
  parse(md: string): import("mdast").Root;
  runSync(tree: import("mdast").Root): Nodes;
  processSync(md: string): { toString(): string };
  use(plugin: unknown, ...settings: unknown[]): this;
}

function buildMdastProcessor(extensions: ExtensionSet[]): TestProcessor {
  let p: TestProcessor = unified().use(remarkParse).use(remarkGfm) as unknown as TestProcessor;
  for (const ext of extensions) {
    if (ext === "math") p = p.use(remarkMath);
    if (ext === "frontmatter") p = p.use(remarkFrontmatter, ["yaml", "toml"]);
    if (ext === "directive") p = p.use(remarkDirective);
  }
  return p;
}

function buildHastProcessor(extensions: ExtensionSet[]): TestProcessor {
  const p = buildMdastProcessor(extensions);
  return p.use(remarkRehype, REF_REHYPE_OPTIONS);
}

function featuresToSatteri(extensions: ExtensionSet[]): Features {
  const features: Features = {};
  for (const ext of extensions) {
    if (ext === "math") features.math = true;
    if (ext === "frontmatter") features.frontmatter = true;
    if (ext === "directive") features.directive = true;
  }
  return features;
}

type AnyNode = Record<string, unknown>;

export function normalizeAlignToStyle(node: AnyNode): AnyNode {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  if (out.properties && typeof out.properties === "object") {
    const props = { ...(out.properties as Record<string, unknown>) };
    if ("align" in props && typeof props.align === "string") {
      props.style = `text-align: ${props.align}`;
      delete props.align;
    }
    out.properties = props;
  }
  if (Array.isArray(out.children)) {
    out.children = (out.children as AnyNode[]).map(normalizeAlignToStyle);
  }
  return out;
}

function serialize(node: unknown): AnyNode {
  return JSON.parse(JSON.stringify(node));
}

function stripData(node: AnyNode): AnyNode {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  delete out.data;
  if (Array.isArray(out.children)) {
    out.children = (out.children as AnyNode[]).map(stripData);
  }
  return out;
}

// Intentional divergence: Sätteri keeps `data.lang` on HAST code elements;
// remark-rehype drops it (the language is already encoded in
// `properties.className`). Strip it from satteri's output before conformance
// comparisons. See website/content/docs/divergences.md.
function stripHastDataLang(node: AnyNode): AnyNode {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  if (out.data && typeof out.data === "object" && "lang" in (out.data as object)) {
    const { lang: _lang, ...rest } = out.data as Record<string, unknown>;
    if (Object.keys(rest).length > 0) {
      out.data = rest;
    } else {
      delete out.data;
    }
  }
  if (Array.isArray(out.children)) {
    out.children = (out.children as AnyNode[]).map(stripHastDataLang);
  }
  return out;
}

export function referenceMdast(md: string): unknown {
  return serialize(mdastProcessor.parse(md));
}

export function referenceHast(md: string): unknown {
  const mdast = hastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(hastProcessor.runSync(mdast) as Nodes));
}

export function satteriMdast(md: string): unknown {
  return serialize(markdownToMdast(md, { features: BASE_FEATURES }));
}

export function satteriHast(md: string): unknown {
  return stripHastDataLang(serialize(markdownToHast(md, { features: BASE_FEATURES })));
}

const mathMdastProcessor = unified().use(remarkParse).use(remarkGfm).use(remarkMath);
const mathHastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype, REF_REHYPE_OPTIONS);
const mathHtmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype, REF_REHYPE_OPTIONS)
  .use(rehypeStringify, { allowDangerousHtml: true });

// Isolate math: the reference math processors don't enable frontmatter.
const MATH_FEATURES: Features = { math: true, frontmatter: false };

export function referenceMathMdast(md: string): unknown {
  return stripData(serialize(mathMdastProcessor.parse(md)));
}

export function satteriMathMdast(md: string): unknown {
  return stripData(serialize(markdownToMdast(md, { features: MATH_FEATURES })));
}

export function referenceMathHast(md: string): unknown {
  const mdast = mathHastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(mathHastProcessor.runSync(mdast) as Nodes));
}

export function referenceMathHtml(md: string): string {
  return normalizeHtmlForComparison(String(mathHtmlProcessor.processSync(md)));
}

export function satteriMathHast(md: string): unknown {
  return stripHastDataLang(serialize(markdownToHast(md, { features: MATH_FEATURES })));
}

export function satteriMathHtml(md: string): string {
  const { html } = markdownToHtml(md, { features: MATH_FEATURES });
  return normalizeHtmlForComparison(html);
}

// singleDollarTextMath: false on both sides, to pin satteri against
// remark-math configured the same way.
const mathNoSingleMdastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath, { singleDollarTextMath: false });
const mathNoSingleHastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath, { singleDollarTextMath: false })
  .use(remarkRehype, REF_REHYPE_OPTIONS);

const MATH_NO_SINGLE_FEATURES: Features = {
  math: { singleDollarTextMath: false },
  frontmatter: false,
};

export function assertNoSingleDollarMathMdastConformance(md: string): void {
  const expected = stripData(serialize(mathNoSingleMdastProcessor.parse(md)));
  const actual = stripData(serialize(markdownToMdast(md, { features: MATH_NO_SINGLE_FEATURES })));
  expect(actual).toEqual(expected);
}

export function assertNoSingleDollarMathHastConformance(md: string): void {
  const mdast = mathNoSingleHastProcessor.parse(md);
  const expected = normalizeAlignToStyle(
    serialize(mathNoSingleHastProcessor.runSync(mdast) as Nodes),
  );
  const actual = stripHastDataLang(
    serialize(markdownToHast(md, { features: MATH_NO_SINGLE_FEATURES })),
  );
  expect(actual).toEqual(expected);
}

// remark-rehype takes callbacks for back-label/back-content; satteri uses
// templates with auto-sup. This helper translates satteri's shape into
// matching remark-rehype callbacks.
const BASE_FOOTNOTE_FEATURES: Features = { math: false, frontmatter: false };

type FootnoteCallback = (referenceNumber: number, rerunIndex: number) => string;

export interface FootnoteOptionsConformance {
  label?: string;
  /**
   * Static text used for every back-content (auto-sup appended for k>1),
   * or a callback returning the per-backref text.
   */
  backContent?: string | FootnoteCallback;
  /**
   * Template with `{reference}` placeholder (`n` for k=1, `n-K` for k>1),
   * or a callback returning the per-backref aria-label.
   */
  backLabel?: string | FootnoteCallback;
}

export function assertFootnoteHastConformance(
  md: string,
  options: FootnoteOptionsConformance = {},
): void {
  const satFeatures: Features = {
    ...BASE_FOOTNOTE_FEATURES,
    gfm: { footnotes: options },
  };
  const actual = stripHastDataLang(serialize(markdownToHast(md, { features: satFeatures })));

  const refOpts: Record<string, unknown> = { ...REF_REHYPE_OPTIONS };
  if (options.label !== undefined) refOpts.footnoteLabel = options.label;
  if (options.backLabel !== undefined) {
    if (typeof options.backLabel === "function") {
      const cb = options.backLabel;
      refOpts.footnoteBackLabel = (refIdx: number, rerefIdx: number) => cb(refIdx + 1, rerefIdx);
    } else {
      const tpl = options.backLabel;
      refOpts.footnoteBackLabel = (refIdx: number, rerefIdx: number) => {
        const ref = rerefIdx > 1 ? `${refIdx + 1}-${rerefIdx}` : `${refIdx + 1}`;
        return tpl.replace("{reference}", ref);
      };
    }
  }
  if (options.backContent !== undefined) {
    if (typeof options.backContent === "function") {
      const cb = options.backContent;
      // Callback mode in satteri skips auto-sup; mirror that here.
      refOpts.footnoteBackContent = (refIdx: number, rerefIdx: number) => [
        { type: "text", value: cb(refIdx + 1, rerefIdx) },
      ];
    } else {
      const content = options.backContent;
      refOpts.footnoteBackContent = (_: number, rerefIdx: number) => {
        const children: unknown[] = [{ type: "text", value: content }];
        if (rerefIdx > 1) {
          children.push({
            type: "element",
            tagName: "sup",
            properties: {},
            children: [{ type: "text", value: String(rerefIdx) }],
          });
        }
        return children;
      };
    }
  }
  const proc = unified().use(remarkParse).use(remarkGfm).use(remarkRehype, refOpts);
  const mdast = proc.parse(md);
  const expected = normalizeAlignToStyle(serialize(proc.runSync(mdast) as Nodes));
  expect(actual).toEqual(expected);
}

const fmMdastProcessor = buildMdastProcessor(["frontmatter"]);
const fmHastProcessor = buildHastProcessor(["frontmatter"]);
const fmHtmlProcessor = buildHastProcessor(["frontmatter"]).use(rehypeStringify, {
  allowDangerousHtml: true,
});
// Isolate frontmatter: the reference fm processors don't enable math.
const FM_FEATURES: Features = { frontmatter: true, math: false };

export function referenceFmMdast(md: string): unknown {
  return serialize(fmMdastProcessor.parse(md));
}

export function referenceFmHast(md: string): unknown {
  const mdast = fmHastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(fmHastProcessor.runSync(mdast) as Nodes));
}

export function referenceFmHtml(md: string): string {
  return normalizeHtmlForComparison(String(fmHtmlProcessor.processSync(md)));
}

export function satteriFmMdast(md: string): unknown {
  return serialize(markdownToMdast(md, { features: FM_FEATURES }));
}

export function satteriFmHast(md: string): unknown {
  return stripHastDataLang(serialize(markdownToHast(md, { features: FM_FEATURES })));
}

export function satteriFmHtml(md: string): string {
  const { html } = markdownToHtml(md, { features: FM_FEATURES });
  return normalizeHtmlForComparison(html);
}

// Find-and-replace autolinks carry source positions here and none in remark.
// Dropping positions wholesale would hide every other position bug, so each
// extra one is verified against the source before being removed.

interface PositionedNode {
  type: string;
  tagName?: string;
  value?: string;
  children?: PositionedNode[];
  position?: { start: { offset: number }; end: { offset: number } };
}

const entityCache = new Map<string, string>();

/** Decode one `&…;` reference, or `undefined` if it isn't one. */
function decodeEntity(raw: string): string | undefined {
  let decoded = entityCache.get(raw);
  if (decoded === undefined) {
    const paragraph = (mdastProcessor.parse(raw).children as unknown as AnyNode[])[0];
    const first = (paragraph?.children as AnyNode[] | undefined)?.[0];
    decoded = first && first.type === "text" ? String(first.value) : raw;
    entityCache.set(raw, decoded);
  }
  return decoded === raw ? undefined : decoded;
}

const ENTITY_RE = /^&(?:#[Xx][0-9A-Fa-f]{1,6}|#\d{1,7}|[A-Za-z][A-Za-z0-9]{0,31});/;

/**
 * Undo the transforms between raw source and a text node's value. Written
 * independently of the parser's own alignment so a symmetric bug can't cancel out.
 */
function decodeRawSlice(raw: string, value: string): string {
  let out = "";
  let i = 0;
  while (i < raw.length) {
    const c = raw[i]!;
    if (c === "&") {
      const match = ENTITY_RE.exec(raw.slice(i));
      const decoded = match ? decodeEntity(match[0]) : undefined;
      if (match && decoded !== undefined) {
        out += decoded;
        i += match[0].length;
        continue;
      }
    } else if (c === "\\" && /[!-/:-@[-`{-~]/.test(raw[i + 1] ?? "")) {
      out += raw[i + 1];
      i += 2;
      continue;
    } else if (c === " " || c === "\t") {
      let j = i;
      while (j < raw.length && (raw[j] === " " || raw[j] === "\t")) j += 1;
      if (raw[j] === "\n" || raw[j] === "\r") {
        i = j;
        continue;
      }
    } else if (c === "\n" || c === "\r") {
      // Both conventions are in use: a `text` keeps the raw line ending, an
      // `inlineCode` normalizes it.
      const ending = c === "\r" && raw[i + 1] === "\n" ? "\r\n" : c;
      out += value.startsWith(ending, out.length) ? ending : "\n";
      i += ending.length;
      // The same characters can be content, so stop stripping the block prefix
      // as soon as the value agrees.
      while (
        i < raw.length &&
        (raw[i] === " " || raw[i] === "\t" || raw[i] === ">") &&
        value[out.length] !== raw[i]
      ) {
        i += 1;
      }
      continue;
    }
    out += c;
    i += 1;
  }
  return out;
}

/** Slicing the source by a node's reported offsets reproduces its raw text. */
function assertSliceInvariant(node: PositionedNode, input: string, label: string): void {
  const { start, end } = node.position!;
  expect(start.offset, `${label}: start offset out of range`).toBeGreaterThanOrEqual(0);
  expect(end.offset, `${label}: end before start`).toBeGreaterThanOrEqual(start.offset);
  expect(end.offset, `${label}: end offset out of range`).toBeLessThanOrEqual(input.length);
  if (typeof node.value === "string") {
    // The span must be exact under one convention or the other: the construct
    // path slices raw source into the value, find-and-replace decodes it.
    const slice = input.slice(start.offset, end.offset);
    if (slice !== node.value) {
      expect(
        decodeRawSlice(slice, node.value),
        `${label}: raw slice does not decode to the node value`,
      ).toBe(node.value);
    }
  }
}

/** Sibling spans are ascending, non-overlapping, and inside their parent's. */
function assertSpanSet(parent: PositionedNode, label: string): void {
  let previousEnd = 0;
  for (const child of parent.children ?? []) {
    if (!child.position) continue;
    expect(
      child.position.start.offset,
      `${label}: overlapping sibling spans`,
    ).toBeGreaterThanOrEqual(previousEnd);
    previousEnd = child.position.end.offset;
    if (parent.position) {
      expect(
        child.position.start.offset,
        `${label}: child starts before its parent`,
      ).toBeGreaterThanOrEqual(parent.position.start.offset);
      expect(
        child.position.end.offset,
        `${label}: child ends after its parent`,
      ).toBeLessThanOrEqual(parent.position.end.offset);
    }
  }
}

function isAutolinkNode(node: PositionedNode): boolean {
  return node.type === "link" || (node.type === "element" && node.tagName === "a");
}

function stripVerifiedSubtree(
  actual: PositionedNode | undefined,
  expected: PositionedNode | undefined,
  input: string,
  label: string,
): void {
  if (!actual || !expected) return;
  // A `link` has no `value`, so the slice invariant only bounds-checks it;
  // sibling ordering is the rest of the check before the position is deleted.
  assertSpanSet(actual, label);
  if (actual.position) {
    assertSliceInvariant(actual, input, label);
    delete actual.position;
  }
  const expectedChildren = expected.children ?? [];
  for (const [ix, child] of (actual.children ?? []).entries()) {
    stripVerifiedSubtree(child, expectedChildren[ix], input, `${label}/${child.type}[${ix}]`);
  }
}

/** The slice invariant on every `link` and `text`: a wrong position is worse
 * than the absent one it replaces. */
export function assertSliceInvariantEverywhere(tree: unknown, input: string): void {
  const walk = (node: PositionedNode, label: string): void => {
    if (node.position && (node.type === "link" || node.type === "text")) {
      assertSliceInvariant(node, input, label);
    }
    for (const [ix, child] of (node.children ?? []).entries()) {
      walk(child, `${label}/${child.type}[${ix}]`);
    }
  };
  if (typeof tree !== "object" || tree === null) return;
  walk(tree as PositionedNode, "root");
}

export function reconcileFnrPositions(actual: unknown, expected: unknown, input: string): void {
  const walk = (a: PositionedNode, e: PositionedNode, label: string): void => {
    const actualChildren = a.children;
    const expectedChildren = e.children;
    if (!Array.isArray(actualChildren) || !Array.isArray(expectedChildren)) return;
    if (actualChildren.length !== expectedChildren.length) return;
    // Scoped to the shape `findAndReplace` produces — a parent holding a
    // position-less link — so a stray position anywhere else still fails.
    const inFnrScope = expectedChildren.some((c) => isAutolinkNode(c) && !c.position);
    if (inFnrScope) assertSpanSet(a, label);
    for (const [ix, expectedChild] of expectedChildren.entries()) {
      const actualChild = actualChildren[ix]!;
      const childLabel = `${label}/${expectedChild.type}[${ix}]`;
      if (inFnrScope && !expectedChild.position) {
        stripVerifiedSubtree(actualChild, expectedChild, input, childLabel);
      } else {
        walk(actualChild, expectedChild, childLabel);
      }
    }
  };
  if (typeof actual !== "object" || actual === null) return;
  if (typeof expected !== "object" || expected === null) return;
  walk(actual as PositionedNode, expected as PositionedNode, "root");
}

export function assertMdastConformance(md: string): void {
  const actual = satteriMdast(md);
  const expected = referenceMdast(md);
  reconcileFnrPositions(actual, expected, md);
  expect(actual).toEqual(expected);
}

/** The part of an mdast node the autolink suites walk. */
export interface UrlNode {
  type: string;
  url?: string;
  children?: UrlNode[];
  position?: { start: { offset: number }; end: { offset: number } };
}

/** Every `link` URL in document order. */
export function collectUrls(tree: unknown): string[] {
  const out: string[] = [];
  const walk = (node: UrlNode): void => {
    if (node.type === "link") out.push(String(node.url));
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree as UrlNode);
  return out;
}

/** Every `link` URL satteri produces for `md`, in document order. */
export function linkUrls(md: string): string[] {
  return collectUrls(satteriMdast(md));
}

/** The tree matches remark, and the autolinks are the ones named. */
export function conforms(md: string, urls: string[]): void {
  assertMdastConformance(md);
  expect(linkUrls(md), JSON.stringify(md)).toEqual(urls);
}

/** Like `assertMdastConformance` but strips `position` fields before
 * comparing. Useful when the structural mdast matches but offsets diverge
 * in non-load-bearing ways (e.g. EOF accounting around trailing blanks). */
export function assertMdastConformanceNoPosition(md: string): void {
  expect(stripPositions(serialize(markdownToMdast(md, { features: BASE_FEATURES })))).toEqual(
    stripPositions(serialize(mdastProcessor.parse(md))),
  );
}

export function assertHastConformance(md: string): void {
  const actual = satteriHast(md);
  const expected = referenceHast(md);
  reconcileFnrPositions(actual, expected, md);
  expect(actual).toEqual(expected);
}

export function assertHtmlConformance(md: string): void {
  expect(satteriHtml(md)).toEqual(referenceHtml(md));
}

export function assertExtMdastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildMdastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const expected = stripData(serialize(proc.parse(md)));
  const actual = stripData(serialize(markdownToMdast(md, { features })));
  reconcileFnrPositions(actual, expected, md);
  expect(actual).toEqual(expected);
}

function stripPositions(node: AnyNode): AnyNode {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  delete out.data;
  delete out.position;
  if (Array.isArray(out.children)) {
    out.children = (out.children as AnyNode[]).map(stripPositions);
  }
  return out;
}

export function assertExtHastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildHastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const mdast = proc.parse(md);
  const expected = normalizeAlignToStyle(serialize(proc.runSync(mdast) as Nodes));
  const actual = stripHastDataLang(serialize(markdownToHast(md, { features })));
  reconcileFnrPositions(actual, expected, md);
  expect(actual).toEqual(expected);
}

function normalizeHtmlForComparison(html: string): string {
  return (
    html
      .replace(/<br>/g, "<br />")
      .replace(/<br\/>/g, "<br />")
      .replace(/<hr>/g, "<hr />")
      .replace(/<hr\/>/g, "<hr />")
      // remark+rehype favours hex entities (`&#x26;`); satteri (and the
      // CommonMark spec) use named ones. Canonicalize to named, then
      // collapse the few entities rehype-stringify never has to encode.
      // The `&quot; → "` collapse is context-unaware and could mask an
      // unescaped `"` inside an attribute value; tolerated until we have
      // an HTML-aware compare.
      .replace(/&#x3C;/g, "&lt;")
      .replace(/&#x3E;/g, "&gt;")
      .replace(/&#x26;/g, "&amp;")
      .replace(/&#x22;/g, "&quot;")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      // remark+rehype emits the legacy `align="X"` attribute on table cells;
      // satteri emits modern `style="text-align: X"`. Canonicalize for diff.
      .replace(/ align="(left|right|center)"/g, ' style="text-align: $1"')
      .trim()
  );
}

export function referenceHtml(md: string): string {
  return normalizeHtmlForComparison(htmlProcessor.processSync(md).toString());
}

export function satteriHtml(md: string): string {
  const { html } = markdownToHtml(md, { features: BASE_FEATURES });
  return normalizeHtmlForComparison(html);
}

// Collapsing whitespace around tags also hides whitespace-only text nodes next
// to an element, such as a table's row newlines: JSX strips those, satteri keeps
// them.
function normalizeHtml(html: string): string {
  return html.replace(/>\s+</g, "><").replace(/\s+</g, "<").replace(/>\s+/g, ">").trim();
}

export async function assertMdxConformance(
  input: string,
  components: Record<string, unknown> = {},
): Promise<void> {
  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...mdxRuntime,
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(
    createElement(MdxComponent as React.FC<Record<string, unknown>>, { components }),
  );

  const { default: SatComponent } = await satteriEvaluate(input, {
    ...satteriRuntime,
  });
  const satHtml = renderToStaticMarkup(
    createElement(SatComponent as React.FC<Record<string, unknown>>, { components }),
  );

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

// Reference is @mdx-js/mdx with `format: "md"`. Both sides evaluate to a
// component and render through react-dom/server, so escaping is identical and
// only structural differences survive.
export interface MarkdownJsConformanceOptions {
  components?: Record<string, unknown>;
  rawHtml?: boolean;
  frontmatter?: boolean;
  math?: boolean;
  /** Pins when raw HTML is dropped: only what the plugins leave behind goes. */
  rewriteRaw?: boolean;
}

/** Makes a `raw` node show up in the render instead of being dropped. */
const rewriteRawToCode = {
  reference: () => (tree: Nodes) => {
    const walk = (node: Nodes): void => {
      if (!("children" in node)) return;
      node.children = node.children.map((child) => {
        walk(child as Nodes);
        return child.type === "raw"
          ? ({
              type: "element",
              tagName: "code",
              properties: {},
              children: [{ type: "text", value: child.value }],
            } as Nodes)
          : child;
      }) as typeof node.children;
    };
    walk(tree);
  },
  satteri: defineHastPlugin({
    name: "rewrite-raw-to-code",
    raw(node) {
      return {
        type: "element",
        tagName: "code",
        properties: {},
        children: [{ type: "text", value: node.value }],
      } as HastNode;
    },
  }),
};

export async function assertMarkdownJsConformance(
  input: string,
  options: MarkdownJsConformanceOptions = {},
): Promise<void> {
  const {
    components = {},
    rawHtml = false,
    frontmatter = false,
    math = false,
    rewriteRaw = false,
  } = options;

  const remarkPlugins: unknown[] = [remarkGfm];
  if (frontmatter) remarkPlugins.push([remarkFrontmatter, ["yaml", "toml"]]);
  if (math) remarkPlugins.push(remarkMath);
  const rehypePlugins: unknown[] = [];
  if (rawHtml) rehypePlugins.push(rehypeRaw);
  if (rewriteRaw) rehypePlugins.push(rewriteRawToCode.reference);
  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...mdxRuntime,
    format: "md",
    remarkPlugins: remarkPlugins as MdxEvaluateOptions["remarkPlugins"],
    rehypePlugins: rehypePlugins as MdxEvaluateOptions["rehypePlugins"],
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(
    createElement(MdxComponent as React.FC<Record<string, unknown>>, { components }),
  );

  const { code } = markdownToJs(input, {
    outputFormat: "function-body",
    features: { frontmatter, math, rawHtml },
    hastPlugins: rewriteRaw ? [rewriteRawToCode.satteri] : [],
  });
  const { default: SatComponent } = new Function(code)(satteriRuntime) as { default: Function };
  const satHtml = renderToStaticMarkup(
    createElement(SatComponent as React.FC<Record<string, unknown>>, { components }),
  );

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

interface ModuleEnvelope {
  pragmas: string[];
  imports: string[];
  defaultExport: string | null;
  markers: string[];
}

// Presence-only, not a text comparison: satteri emits `Object.assign` where
// @mdx-js/mdx spreads, and pretty-prints differently.
const ENVELOPE_MARKERS = [
  "_createMdxContent",
  "MDXContent",
  "MDXLayout",
  "_provideComponents",
  "_missingMdxReference",
  "props.components",
  "_Fragment",
  "_jsx",
  "_jsxs",
  "_jsxDEV",
  "React.createElement",
];

function moduleEnvelope(code: string): ModuleEnvelope {
  const imports: string[] = [];
  const importRe = /import\s+(?:([\w$]+)\s*,?\s*)?(?:\{([^}]*)\})?\s*from\s*"([^"]+)"/g;
  for (const match of code.matchAll(importRe)) {
    const names: string[] = [];
    if (match[1]) names.push(`default as ${match[1]}`);
    if (match[2])
      names.push(
        ...match[2]
          .split(",")
          .map((name) => name.trim().replace(/\s+/g, " "))
          .filter(Boolean),
      );
    imports.push(`${match[3]}: ${names.sort().join(", ")}`);
  }
  const defaultExport = /export default (?:function\s+)?([\w$]+)/.exec(code);
  return {
    pragmas: [...code.matchAll(/\/\*(@jsx[A-Za-z]*\s[^*]*)\*\//g)].map((match) => match[1]!.trim()),
    imports: imports.sort(),
    defaultExport: defaultExport ? defaultExport[1]! : null,
    markers: ENVELOPE_MARKERS.filter((marker) =>
      new RegExp(`${marker.replaceAll(".", "\\.")}\\b`).test(code),
    ),
  };
}

/**
 * Compare the compiled module's envelope against @mdx-js/mdx `format: "md"`.
 * Covers the options that shape the module rather than the rendered tree, which
 * the evaluate-and-render comparison cannot see.
 */
export async function assertMarkdownJsModuleConformance(
  input: string,
  options: MarkdownToJsOptions & { frontmatter?: boolean } = {},
): Promise<void> {
  const { frontmatter = false, features, ...jsOptions } = options;
  const remarkPlugins: unknown[] = [remarkGfm];
  if (frontmatter) remarkPlugins.push([remarkFrontmatter, ["yaml", "toml"]]);

  const expected = moduleEnvelope(
    String(
      await mdxCompile(input, {
        format: "md",
        remarkPlugins: remarkPlugins as MdxCompileOptions["remarkPlugins"],
        ...(jsOptions as MdxCompileOptions),
      }),
    ),
  );
  const { code } = markdownToJs(input, {
    ...jsOptions,
    features: { frontmatter, math: false, ...features },
  });
  expect(moduleEnvelope(code)).toEqual(expected);
}

/**
 * Compare the `__source` metadata against @mdx-js/mdx `format: "md"`: one
 * `line:column` per JSX call, in source order.
 */
export async function assertMarkdownJsDevPositionConformance(input: string): Promise<void> {
  const positions = (code: string): string[] =>
    [...code.matchAll(/lineNumber: (\d+),\s*columnNumber: (\d+)/g)].map(
      (match) => `${match[1]}:${match[2]}`,
    );

  const expected = positions(
    String(
      await mdxCompile(input, {
        format: "md",
        development: true,
        remarkPlugins: [remarkGfm] as MdxCompileOptions["remarkPlugins"],
      }),
    ),
  );
  const { code } = markdownToJs(input, {
    development: true,
    features: { frontmatter: false, math: false },
  });
  expect(expected.length).toBeGreaterThan(0);
  expect(positions(code)).toEqual(expected);
}

// Like `assertMdxConformance`, but with math enabled on both pipelines
// (satteri `features.math`, reference `remark-math`). Exercises how MDX
// expressions and `$...$` math interact, e.g. that braces inside a math span
// stay math text rather than being parsed as an expression.
export async function assertMdxMathConformance(
  input: string,
  components: Record<string, unknown> = {},
): Promise<void> {
  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...mdxRuntime,
    remarkPlugins: [remarkMath],
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(
    createElement(MdxComponent as React.FC<Record<string, unknown>>, { components }),
  );

  const { default: SatComponent } = await satteriEvaluate(input, {
    ...satteriRuntime,
    features: { math: true },
  });
  const satHtml = renderToStaticMarkup(
    createElement(SatComponent as React.FC<Record<string, unknown>>, { components }),
  );

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

// Set an inline `style` string on every `<tag>` element via a hast/rehype
// plugin on both pipelines, evaluate, and compare the rendered HTML. This is
// the path expressive-code (and similar hast plugins) take: satteri's HAST→JSX
// compiler parses `style="…"` into a JSX style object, which must agree with
// @mdx-js/mdx (hast-util-to-estree). CSS custom properties are case-sensitive,
// so casing like `--tmLabel` must survive intact on both sides.
export async function assertMdxInlineStyleConformance(
  input: string,
  tag: string,
  style: string,
): Promise<void> {
  const setStyle = (node: AnyNode): void => {
    if (node.type === "element" && node.tagName === tag) {
      node.properties = { ...(node.properties as AnyNode), style };
    }
    if (Array.isArray(node.children)) {
      for (const child of node.children as AnyNode[]) setStyle(child);
    }
  };
  const rehypeSetStyle = () => (tree: Nodes) => setStyle(tree as unknown as AnyNode);
  const satteriSetStyle = defineHastPlugin({
    name: "set-inline-style",
    element: {
      filter: [tag],
      visit(node, ctx) {
        ctx.setProperty(node, "style", style);
      },
    },
  });

  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...mdxRuntime,
    rehypePlugins: [rehypeSetStyle],
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(createElement(MdxComponent as React.FC));

  const { default: SatComponent } = await satteriEvaluate(input, {
    ...satteriRuntime,
    hastPlugins: [satteriSetStyle],
  });
  const satHtml = renderToStaticMarkup(createElement(SatComponent as React.FC));

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

export async function assertBothReject(input: string): Promise<void> {
  let mdxOk = true;
  try {
    await mdxEvaluate(input, { ...mdxRuntime });
  } catch {
    mdxOk = false;
  }

  let satteriOk = true;
  try {
    mdxToJs(input);
  } catch {
    satteriOk = false;
  }

  expect(satteriOk).toBe(mdxOk);
}

export async function assertRejects(input: string): Promise<void> {
  expect(() => mdxToJs(input)).toThrow();
}
