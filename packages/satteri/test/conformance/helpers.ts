import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { evaluate as satteriEvaluate, markdownToMdast, markdownToHast, markdownToHtml, mdxToJs } from "../../src/index.js";
import type { Features } from "../../src/index.js";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import * as runtime from "react/jsx-runtime";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";
import { remarkDefinitionList, defListHastHandlers } from "remark-definition-list";
import { remarkAlert } from "remark-github-blockquote-alert";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Nodes } from "hast";
import type { Processor } from "unified";
import { expect } from "vitest";

const mdastProcessor = unified().use(remarkParse).use(remarkGfm);
const hastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkRehype, { allowDangerousHtml: true });
const htmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeStringify, { allowDangerousHtml: true });

export type ExtensionSet = "math" | "frontmatter" | "directive" | "definitionList" | "gfmAlerts";

function buildMdastProcessor(extensions: ExtensionSet[]): Processor {
  let p = unified().use(remarkParse).use(remarkGfm);
  for (const ext of extensions) {
    if (ext === "math") p = p.use(remarkMath);
    if (ext === "frontmatter") p = p.use(remarkFrontmatter, ["yaml", "toml"]);
    if (ext === "directive") p = p.use(remarkDirective);
    if (ext === "definitionList") p = p.use(remarkDefinitionList);
    if (ext === "gfmAlerts") p = p.use(remarkAlert);
  }
  return p;
}

function buildHastProcessor(extensions: ExtensionSet[]): Processor {
  let p = buildMdastProcessor(extensions);
  const handlers: Record<string, unknown> = {};
  if (extensions.includes("definitionList")) {
    Object.assign(handlers, defListHastHandlers);
  }
  return p.use(remarkRehype, { allowDangerousHtml: true, handlers: Object.keys(handlers).length > 0 ? handlers : undefined } as any);
}

function featuresToSatteri(extensions: ExtensionSet[]): Features {
  const features: Features = {};
  for (const ext of extensions) {
    if (ext === "math") features.math = true;
    if (ext === "frontmatter") features.frontmatter = true;
    if (ext === "directive") features.directive = true;
    if (ext === "definitionList") features.definitionList = true;
    if (ext === "gfmAlerts") features.githubAlerts = true;
  }
  return features;
}

type AnyNode = Record<string, unknown>;

export function normalizeAlignToStyle(node: AnyNode): AnyNode {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  delete out.data;
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

function serialize(node: unknown): unknown {
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

export function referenceMdast(md: string): unknown {
  return serialize(mdastProcessor.parse(md));
}

export function referenceHast(md: string): unknown {
  const mdast = hastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(hastProcessor.runSync(mdast) as Nodes));
}

export function satteriMdast(md: string): unknown {
  return serialize(markdownToMdast(md));
}

export function satteriHast(md: string): unknown {
  return serialize(markdownToHast(md));
}

const mathMdastProcessor = unified().use(remarkParse).use(remarkGfm).use(remarkMath);
const mathHastProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype, { allowDangerousHtml: true });
const mathHtmlProcessor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype, { allowDangerousHtml: true })
  .use(rehypeStringify, { allowDangerousHtml: true });

const MATH_FEATURES: Features = { math: true };

export function referenceMathMdast(md: string): unknown {
  return stripData(serialize(mathMdastProcessor.parse(md)) as AnyNode);
}

export function satteriMathMdast(md: string): unknown {
  return stripData(serialize(markdownToMdast(md, { features: MATH_FEATURES })) as AnyNode);
}

export function referenceMathHast(md: string): unknown {
  const mdast = mathHastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(mathHastProcessor.runSync(mdast) as Nodes));
}

export function referenceMathHtml(md: string): string {
  return normalizeHtmlForComparison(mathHtmlProcessor.processSync(md).toString());
}

export function satteriMathHast(md: string): unknown {
  return serialize(markdownToHast(md, { features: MATH_FEATURES }));
}

export function satteriMathHtml(md: string): string {
  return normalizeHtmlForComparison(markdownToHtml(md, { features: MATH_FEATURES }));
}

const fmMdastProcessor = buildMdastProcessor(["frontmatter"]);
const fmHastProcessor = buildHastProcessor(["frontmatter"]);
const fmHtmlProcessor = buildHastProcessor(["frontmatter"])
  .use(rehypeStringify, { allowDangerousHtml: true });
const FM_FEATURES: Features = { frontmatter: true };

export function referenceFmMdast(md: string): unknown {
  return serialize(fmMdastProcessor.parse(md));
}

export function referenceFmHast(md: string): unknown {
  const mdast = fmHastProcessor.parse(md);
  return normalizeAlignToStyle(serialize(fmHastProcessor.runSync(mdast) as Nodes));
}

export function referenceFmHtml(md: string): string {
  return normalizeHtmlForComparison(fmHtmlProcessor.processSync(md).toString());
}

export function satteriFmMdast(md: string): unknown {
  return serialize(markdownToMdast(md, { features: FM_FEATURES }));
}

export function satteriFmHast(md: string): unknown {
  return serialize(markdownToHast(md, { features: FM_FEATURES }));
}

export function satteriFmHtml(md: string): string {
  return normalizeHtmlForComparison(markdownToHtml(md, { features: FM_FEATURES }));
}

export function assertMdastConformance(md: string): void {
  expect(satteriMdast(md)).toEqual(referenceMdast(md));
}

export function assertHastConformance(md: string): void {
  expect(satteriHast(md)).toEqual(referenceHast(md));
}

export function assertExtMdastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildMdastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const expected = stripData(serialize(proc.parse(md)) as AnyNode);
  const actual = stripData(serialize(markdownToMdast(md, { features })) as AnyNode);
  expect(actual).toEqual(expected);
}

export function assertExtHastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildHastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const mdast = proc.parse(md);
  const expected = normalizeAlignToStyle(serialize(proc.runSync(mdast) as Nodes));
  const actual = serialize(markdownToHast(md, { features }));
  expect(actual).toEqual(expected);
}

function normalizeHtmlForComparison(html: string): string {
  return html
    .replace(/<br>/g, "<br />")
    .replace(/<br\/>/g, "<br />")
    .replace(/<hr>/g, "<hr />")
    .replace(/<hr\/>/g, "<hr />")
    .replace(/&#x3C;/g, "&lt;")
    .replace(/&gt;/g, ">")
    .trim();
}

export function referenceHtml(md: string): string {
  return normalizeHtmlForComparison(
    htmlProcessor.processSync(md).toString(),
  );
}

export function satteriHtml(md: string): string {
  const result = markdownToHtml(md);
  if (typeof result !== "string") throw new Error("markdownToHtml returned a promise");
  return normalizeHtmlForComparison(result);
}

function normalizeHtml(html: string): string {
  return html.replace(/>\s+</g, "><").replace(/\s+</g, "<").replace(/>\s+/g, ">").trim();
}

export async function assertMdxConformance(
  input: string,
  components: Record<string, unknown> = {},
): Promise<void> {
  const { default: MdxComponent } = (await mdxEvaluate(input, {
    ...runtime,
  })) as { default: Function };
  const mdxHtml = renderToStaticMarkup(createElement(MdxComponent as any, { components }));

  const { default: SatComponent } = await satteriEvaluate(input, {
    ...runtime,
  });
  const satHtml = renderToStaticMarkup(createElement(SatComponent as any, { components }));

  expect(normalizeHtml(satHtml)).toBe(normalizeHtml(mdxHtml));
}

export async function assertBothReject(input: string): Promise<void> {
  let mdxOk = true;
  try {
    await mdxEvaluate(input, { ...runtime });
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
