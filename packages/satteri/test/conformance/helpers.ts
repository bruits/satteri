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

// Empty reference handlers match Sätteri’s omission of unhandled directives.
const emptyHandler = () => undefined;
export const REF_REHYPE_OPTIONS = {
  allowDangerousHtml: true,
  handlers: {
    containerDirective: emptyHandler,
    leafDirective: emptyHandler,
    textDirective: emptyHandler,
  },
} as const;

// Disable frontmatter in the base reference because failed detection can change unrelated block parsing.
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

// Ignore data.lang because only Sätteri retains it on HAST code elements.
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

// Convert templates to callbacks so remark-rehype uses the same backref text as Sätteri.
const BASE_FOOTNOTE_FEATURES: Features = { math: false, frontmatter: false };

type FootnoteCallback = (referenceNumber: number, rerunIndex: number) => string;

export interface FootnoteOptionsConformance {
  label?: string;
  // String content receives a repeat-reference superscript; callback results do not.
  backContent?: string | FootnoteCallback;
  // The reference placeholder uses n for the first backref and n-K for repeats.
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
      // Callback results must bypass the automatic repeat-reference superscript.
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

// Verify extra autolink positions before removing them so normalization cannot hide incorrect spans.

interface PositionedNode {
  type: string;
  tagName?: string;
  value?: string;
  children?: PositionedNode[];
  position?: { start: { offset: number }; end: { offset: number } };
}

const entityCache = new Map<string, string>();

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

// Keep source alignment independent of the parser so a shared bug cannot cancel out.
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

function assertSliceInvariant(node: PositionedNode, input: string, label: string): void {
  const { start, end } = node.position!;
  expect(start.offset, `${label}: start offset out of range`).toBeGreaterThanOrEqual(0);
  expect(end.offset, `${label}: end before start`).toBeGreaterThanOrEqual(start.offset);
  expect(end.offset, `${label}: end offset out of range`).toBeLessThanOrEqual(input.length);
  if (typeof node.value === "string") {
    const slice = input.slice(start.offset, end.offset);
    if (slice !== node.value) {
      expect(
        decodeRawSlice(slice, node.value),
        `${label}: raw slice does not decode to the node value`,
      ).toBe(node.value);
    }
  }
}

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
  // Links have no value to check, so verify sibling ordering before removing their positions.
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
    // Scoped to the shape `findAndReplace` produces (a parent holding a
    // position-less link), so a stray position anywhere else still fails.
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

const cmarkMdastProcessor = unified().use(remarkParse);
const CMARK_FEATURES: Features = {
  gfm: false,
  frontmatter: false,
  math: false,
  headingAttributes: false,
};

export function assertCommonMarkMdastConformance(md: string): void {
  expect(serialize(markdownToMdast(md, { features: CMARK_FEATURES }))).toEqual(
    serialize(cmarkMdastProcessor.parse(md)),
  );
}

export interface UrlNode {
  type: string;
  url?: string;
  children?: UrlNode[];
  position?: { start: { offset: number }; end: { offset: number } };
}

export function collectUrls(tree: unknown): string[] {
  const out: string[] = [];
  const walk = (node: UrlNode): void => {
    if (node.type === "link") out.push(String(node.url));
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree as UrlNode);
  return out;
}

export function linkUrls(md: string): string[] {
  return collectUrls(satteriMdast(md));
}

export function conforms(md: string, urls: string[]): void {
  assertMdastConformance(md);
  expect(linkUrls(md), JSON.stringify(md)).toEqual(urls);
}

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
      // Entity normalization is context-blind and cannot verify quote escaping inside attribute values.
      .replace(/&#x3C;/g, "&lt;")
      .replace(/&#x3E;/g, "&gt;")
      .replace(/&#x26;/g, "&amp;")
      .replace(/&#x22;/g, "&quot;")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      // Treat CSS text-align and the deprecated align attribute as equivalent table alignment.
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

// Whitespace normalization also hides whitespace-only text nodes between elements.
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

export interface MdxPluginConformanceOptions {
  reference: Pick<MdxCompileOptions, "remarkPlugins" | "rehypePlugins">;
  satteri: Pick<MarkdownToJsOptions, "mdastPlugins" | "hastPlugins">;
  components?: Record<string, unknown>;
}

// Pass `components` overrides: a tag that skips `_components` ignores them.
export async function assertMdxPluginConformance(
  input: string,
  options: MdxPluginConformanceOptions,
): Promise<void> {
  const { reference, satteri, components = {} } = options;

  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...mdxRuntime,
    ...reference,
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(
    createElement(MdxComponent as React.FC<Record<string, unknown>>, { components }),
  );

  const { default: SatComponent } = await satteriEvaluate(input, {
    ...satteriRuntime,
    ...satteri,
  });
  const satHtml = renderToStaticMarkup(
    createElement(SatComponent as React.FC<Record<string, unknown>>, { components }),
  );

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

export interface MarkdownJsConformanceOptions {
  components?: Record<string, unknown>;
  rawHtml?: boolean;
  frontmatter?: boolean;
  math?: boolean;
  rewriteRaw?: boolean;
}

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

// Check presence because valid generated modules can differ in formatting and spread syntax.
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
  const { code } = await markdownToJs(input, {
    ...jsOptions,
    features: { frontmatter, math: false, ...features },
  });
  expect(moduleEnvelope(code)).toEqual(expected);
}

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

export async function assertMdxDevPositionConformance(input: string): Promise<void> {
  const positions = (code: string): string[] =>
    [...code.matchAll(/lineNumber: (\d+),\s*columnNumber: (\d+)/g)].map(
      (match) => `${match[1]}:${match[2]}`,
    );
  // Sorted: the two pipelines emit the reference guards in different orders.
  const missingRefPlaces = (code: string): string[] =>
    [...code.matchAll(/_missingMdxReference\("[^"]*", \w+, "([^"]*)"\)/g)]
      .map((match) => match[1]!)
      .sort();

  const expected = String(await mdxCompile(input, { development: true }));
  const { code } = mdxToJs(input, { development: true });

  expect(positions(expected).length).toBeGreaterThan(0);
  expect(positions(code)).toEqual(positions(expected));
  expect(missingRefPlaces(code)).toEqual(missingRefPlaces(expected));
}

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

  expect(mdxOk).toBe(false);
  expect(satteriOk).toBe(false);
}

export function assertRejects(input: string): void {
  expect(() => mdxToJs(input)).toThrow();
}
