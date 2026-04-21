import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import { evaluate as satteriEvaluate, markdownToMdast, markdownToHast, markdownToHtml, mdxToJs } from "../../src/index.js";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import * as runtime from "react/jsx-runtime";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import type { Nodes } from "hast";
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

export function assertMdastConformance(md: string): void {
  expect(satteriMdast(md)).toEqual(referenceMdast(md));
}

export function assertHastConformance(md: string): void {
  expect(satteriHast(md)).toEqual(referenceHast(md));
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
