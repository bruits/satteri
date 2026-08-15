import {
  markdownToHtml,
  markdownToJs,
  mdxToJs,
  defineMdastPlugin,
  defineHastPlugin,
} from "../src/index.js";
import type {
  CompileOptions,
  MarkdownToHtmlResult,
  MarkdownToJsOptions,
  MarkdownToJsResult,
  MdxCompileOptions,
  MdxToJsResult,
} from "../src/index.js";

type Expect<T extends true> = T;
type Exactly<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;

const syncMdast = defineMdastPlugin({
  name: "sync-mdast",
  code() {},
});

const asyncMdast = defineMdastPlugin({
  name: "async-mdast",
  async code() {
    await Promise.resolve();
  },
});

const syncHast = defineHastPlugin({
  name: "sync-hast",
  text() {},
});

const asyncHast = defineHastPlugin({
  name: "async-hast",
  async text() {
    await Promise.resolve();
  },
});

// A general options type says nothing about whether a plugin is async.
function renderHtml(source: string, options: CompileOptions) {
  return markdownToHtml(source, options);
}
export type _WrapperHtml = Expect<
  Exactly<ReturnType<typeof renderHtml>, MarkdownToHtmlResult | Promise<MarkdownToHtmlResult>>
>;

function renderMdx(source: string, options: MdxCompileOptions) {
  return mdxToJs(source, options);
}
export type _WrapperMdx = Expect<
  Exactly<ReturnType<typeof renderMdx>, MdxToJsResult | Promise<MdxToJsResult>>
>;

function renderMarkdownJs(source: string, options: MarkdownToJsOptions) {
  return markdownToJs(source, options);
}
export type _WrapperMarkdownJs = Expect<
  Exactly<ReturnType<typeof renderMarkdownJs>, MarkdownToJsResult | Promise<MarkdownToJsResult>>
>;

const generalList = markdownToHtml("x", {
  mdastPlugins: [] as NonNullable<CompileOptions["mdastPlugins"]>,
});
export type _GeneralList = Expect<
  Exactly<typeof generalList, MarkdownToHtmlResult | Promise<MarkdownToHtmlResult>>
>;

const noOptions = markdownToHtml("x");
export type _NoOptions = Expect<Exactly<typeof noOptions, MarkdownToHtmlResult>>;

const emptyOptions = markdownToHtml("x", {});
export type _EmptyOptions = Expect<Exactly<typeof emptyOptions, MarkdownToHtmlResult>>;

const featuresOnly = markdownToHtml("x", { features: { gfm: true } });
export type _FeaturesOnly = Expect<Exactly<typeof featuresOnly, MarkdownToHtmlResult>>;

const literalSync = markdownToHtml("x", {
  mdastPlugins: [syncMdast],
  hastPlugins: [syncHast],
});
export type _LiteralSync = Expect<Exactly<typeof literalSync, MarkdownToHtmlResult>>;

const literalAsyncMdast = markdownToHtml("x", { mdastPlugins: [syncMdast, asyncMdast] });
export type _LiteralAsyncMdast = Expect<
  Exactly<typeof literalAsyncMdast, Promise<MarkdownToHtmlResult>>
>;

const literalAsyncHast = markdownToJs("x", { hastPlugins: [asyncHast] });
export type _LiteralAsyncHast = Expect<
  Exactly<typeof literalAsyncHast, Promise<MarkdownToJsResult>>
>;

const literalAsyncMdx = mdxToJs("x", { mdastPlugins: [asyncMdast] });
export type _LiteralAsyncMdx = Expect<Exactly<typeof literalAsyncMdx, Promise<MdxToJsResult>>>;

async function consumesWidened(options: CompileOptions) {
  const { html } = await markdownToHtml("x", options);
  return html satisfies string;
}
export type _AwaitNarrows = Expect<Exactly<Awaited<ReturnType<typeof consumesWidened>>, string>>;
