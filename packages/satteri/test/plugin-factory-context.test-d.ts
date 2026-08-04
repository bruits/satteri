import { markdownToHtml, mdxToJs, defineMdastPlugin, defineHastPlugin } from "../src/index.js";
import type { PluginFactoryContext } from "../src/index.js";

type Expect<T extends true> = T;
type ExpectFalse<T extends false> = T;
type IsPromise<T> = [T] extends [Promise<unknown>] ? true : false;

const asyncMdast = defineMdastPlugin({
  name: "async-mdast",
  async code() {
    await Promise.resolve();
  },
});

const syncMdast = defineMdastPlugin({
  name: "sync-mdast",
  code() {},
});

const asyncHast = defineHastPlugin({
  name: "async-hast",
  async text() {
    await Promise.resolve();
  },
});

// `ResolveInput` matches factories structurally. A factory taking a ctx is not
// assignable to `() => infer Def`, so if that pattern loses its parameters the
// async plugin behind it goes unseen and these flip to `false`.
const ctxFactoryAsync = markdownToHtml("x", {
  mdastPlugins: [(ctx: PluginFactoryContext) => (ctx.sourceFormat === "mdx" ? asyncMdast : null)],
});
export type _CtxFactoryAsync = Expect<IsPromise<typeof ctxFactoryAsync>>;

const ctxFactoryAsyncHast = markdownToHtml("x", {
  hastPlugins: [(ctx: PluginFactoryContext) => (ctx.source ? asyncHast : undefined)],
});
export type _CtxFactoryAsyncHast = Expect<IsPromise<typeof ctxFactoryAsyncHast>>;

const ctxFactoryBundleAsync = mdxToJs("x", {
  mdastPlugins: [(ctx: PluginFactoryContext) => (ctx.fileURL ? [syncMdast, asyncMdast] : false)],
});
export type _CtxFactoryBundleAsync = Expect<IsPromise<typeof ctxFactoryBundleAsync>>;

const nestedCtxFactoryAsync = markdownToHtml("x", {
  mdastPlugins: [[syncMdast, [(ctx: PluginFactoryContext) => (ctx.data ? asyncMdast : null)]]],
});
export type _NestedCtxFactoryAsync = Expect<IsPromise<typeof nestedCtxFactoryAsync>>;

// A zero-argument factory keeps working, and keeps narrowing.
const legacyFactoryAsync = markdownToHtml("x", { mdastPlugins: [() => asyncMdast] });
export type _LegacyFactoryAsync = Expect<IsPromise<typeof legacyFactoryAsync>>;

// The other direction: nothing async in the list must stay synchronous, so the
// fix above cannot pass by making everything a Promise.
const ctxFactorySync = markdownToHtml("x", {
  mdastPlugins: [(ctx: PluginFactoryContext) => (ctx.source ? syncMdast : null)],
});
export type _CtxFactorySync = ExpectFalse<IsPromise<typeof ctxFactorySync>>;

const skipOnly = markdownToHtml("x", {
  mdastPlugins: [null, undefined, false, () => null],
  hastPlugins: [false],
});
export type _SkipOnlySync = ExpectFalse<IsPromise<typeof skipOnly>>;

const plainSync = markdownToHtml("x", { mdastPlugins: [syncMdast] });
export type _PlainSync = ExpectFalse<IsPromise<typeof plainSync>>;

// Skip values must be accepted wherever an entry is, including from a factory.
markdownToHtml("x", {
  mdastPlugins: [null, undefined, false, syncMdast, [null, syncMdast], () => undefined],
  hastPlugins: [false, () => null],
});

// The ctx is read-only enough to be useful without a cast.
markdownToHtml("x", {
  mdastPlugins: [
    (ctx: PluginFactoryContext) => {
      const url: URL | undefined = ctx.fileURL;
      const format: "markdown" | "mdx" = ctx.sourceFormat;
      const src: string = ctx.source;
      const bag: Record<string, unknown> = ctx.data;
      return url && format && src && bag ? syncMdast : null;
    },
  ],
});

// A factory may take no parameter at all.
markdownToHtml("x", { mdastPlugins: [() => syncMdast] });

// On its own, a factory's ctx is contextually typed and needs no annotation.
markdownToHtml("x", {
  mdastPlugins: [({ sourceFormat }) => (sourceFormat === "mdx" ? syncMdast : null)],
});
markdownToHtml("x", { mdastPlugins: [(ctx) => (ctx.source ? syncMdast : null)] });
