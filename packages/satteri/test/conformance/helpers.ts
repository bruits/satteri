import { evaluate as mdxEvaluate } from "@mdx-js/mdx";
import type { EvaluateOptions as MdxEvaluateOptions } from "@mdx-js/mdx";
import {
  evaluate as satteriEvaluate,
  defineHastPlugin,
  markdownToMdast,
  markdownToHast,
  markdownToHtml,
  mdxToJs,
} from "../../src/index.js";
import type { Features, EvaluateOptions } from "../../src/index.js";
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

export function assertMdastConformance(md: string): void {
  expect(satteriMdast(md)).toEqual(referenceMdast(md));
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
  expect(satteriHast(md)).toEqual(referenceHast(md));
}

export function assertHtmlConformance(md: string): void {
  expect(satteriHtml(md)).toEqual(referenceHtml(md));
}

export function assertExtMdastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildMdastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const expected = stripData(serialize(proc.parse(md)));
  const actual = stripData(serialize(markdownToMdast(md, { features })));
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

/** Like `assertExtMdastConformance` but ignores `position`. Use this when the
 * input contains non-ASCII characters: remark counts columns in code points
 * while satteri currently counts in bytes, so positions diverge even when
 * trees are structurally identical. */
export function assertExtMdastConformanceNoPosition(md: string, extensions: ExtensionSet[]): void {
  const proc = buildMdastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const expected = stripPositions(serialize(proc.parse(md)));
  const actual = stripPositions(serialize(markdownToMdast(md, { features })));
  expect(actual).toEqual(expected);
}

export function assertExtHastConformance(md: string, extensions: ExtensionSet[]): void {
  const proc = buildHastProcessor(extensions);
  const features = featuresToSatteri(extensions);
  const mdast = proc.parse(md);
  const expected = normalizeAlignToStyle(serialize(proc.runSync(mdast) as Nodes));
  const actual = stripHastDataLang(serialize(markdownToHast(md, { features })));
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
