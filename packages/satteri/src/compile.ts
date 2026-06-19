import { visitHastHandle, resolveSubscriptions, type HastHandle } from "./hast/hast-visitor.js";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastHandle,
  type MdastPluginInstance,
} from "./mdast/mdast-visitor.js";
import type {
  MdastPluginDefinition,
  HastPluginDefinition,
  MdastPluginInput,
  HastPluginInput,
} from "./plugin.js";
import {
  createHastHandle,
  createMdxHastHandle,
  renderHandle,
  compileHandle,
  dropHandle,
  createMdastHandle,
  createMdxMdastHandle,
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  getHandleSource,
  getMdastFrontmatter,
  serializeHandle,
} from "#binding";
import { MdastReader } from "./mdast/mdast-reader.js";
import { materializeMdastTree } from "./mdast/mdast-materializer.js";
import { markHandleMutated } from "./lazy-child-resolver.js";
import { HastReader } from "./hast/hast-reader.js";
import { materializeHastTree } from "./hast/hast-materializer.js";
import type { MdastNode, HastNode, Data } from "./types.js";

type NativeFeaturesPair = {
  features: Record<string, unknown> | undefined;
  convertOptions: Record<string, unknown> | undefined;
};

/**
 * Split the user-facing `Features` (with nested unions) into the flat napi
 * `JsFeatures` shape plus the conversion-side `JsConvertOptions` carrying
 * the footnote i18n strings. The public API only exposes `features`; the
 * footnote strings are routed to napi internally.
 */
function featuresToNative(features: Features | undefined): NativeFeaturesPair {
  if (!features) return { features: undefined, convertOptions: undefined };
  const result: Record<string, unknown> = {};
  let convertOptions: Record<string, unknown> | undefined;

  if (features.gfm !== undefined) {
    if (typeof features.gfm === "object") {
      const g = features.gfm;
      const gfmOpts: Record<string, unknown> = {};
      if (g.footnotes !== undefined) {
        if (typeof g.footnotes === "object") {
          gfmOpts.footnotes = true;
          convertOptions = convertOptions ?? {};
          if (g.footnotes.label !== undefined) convertOptions.footnoteLabel = g.footnotes.label;
          if (g.footnotes.backContent !== undefined)
            convertOptions.footnoteBackContent = g.footnotes.backContent;
          if (g.footnotes.backLabel !== undefined)
            convertOptions.footnoteBackLabel = g.footnotes.backLabel;
        } else {
          gfmOpts.footnotes = g.footnotes;
        }
      }
      result.gfmOptions = gfmOpts;
    } else {
      result.gfm = features.gfm;
    }
  }
  if (features.frontmatter !== undefined) result.frontmatter = features.frontmatter;
  if (features.math !== undefined) {
    if (typeof features.math === "object") {
      const mathOpts: Record<string, unknown> = {};
      if (features.math.singleDollarTextMath !== undefined)
        mathOpts.singleDollarTextMath = features.math.singleDollarTextMath;
      result.mathOptions = mathOpts;
    } else {
      result.math = features.math;
    }
  }
  if (features.headingAttributes !== undefined)
    result.headingAttributes = features.headingAttributes;
  if (features.directive !== undefined) result.directive = features.directive;
  if (features.superscript !== undefined) result.superscript = features.superscript;
  if (features.subscript !== undefined) result.subscript = features.subscript;
  if (features.wikilinks !== undefined) result.wikilinks = features.wikilinks;
  if (features.smartPunctuation !== undefined) {
    if (typeof features.smartPunctuation === "object") {
      result.smartPunctuationOptions = features.smartPunctuation;
    } else {
      result.smartPunctuation = features.smartPunctuation;
    }
  }
  return { features: result, convertOptions };
}

type MdastPipelineResult = { handle: MdastHandle };

/** Free a handle's arena. A plugin's visitor can hand child stubs to user code;
 *  pass `invalidateStubs` so the epoch bump makes any stub retained past this
 *  point hit the designed retention error instead of snapshotting the freed arena. */
function releaseHandle(handle: AnyHandle, invalidateStubs: boolean): void {
  if (invalidateStubs) markHandleMutated(handle);
  dropHandle(handle);
}

function warnDroppedTransforms(
  plugin: { name?: string },
  dropped: number,
  kind: "mdast" | "hast",
): void {
  const name = plugin.name ?? "<anonymous>";
  const noun = dropped === 1 ? "transform" : "transforms";
  console.warn(
    `satteri: plugin "${name}" queued ${dropped} ${kind} ${noun} on node(s) that were removed or ` +
      `replaced earlier in the same pass; ${dropped === 1 ? "it was" : "they were"} dropped.`,
  );
}

function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginInput[],
  fileURL: URL | undefined,
  data: Data,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  // Each plugin runs once over the tree. A transform that passes a child
  // through (returning it inside the replacement) keeps that child's identity,
  // so a patch the same pass queued on it still applies — nesting composes in
  // one pass. A plugin's own freshly-built nodes are not re-walked; transform
  // them up front, or hand off to a later plugin that sees the materialized tree.
  const runPlugin = (plugin: MdastPluginInstance): void | Promise<void> => {
    const subs = resolveMdastSubscriptions(plugin);
    const result = visitMdastHandle(
      handle,
      plugin,
      subs,
      () => getHandleSource(handle),
      fileURL,
      data,
    );
    const apply = (r: { commandBuffer: Uint8Array; hasMutations: boolean }): void => {
      if (!r.hasMutations) return;
      markHandleMutated(handle);
      const dropped = applyCommandsToMdastHandle(handle, r.commandBuffer);
      if (dropped) warnDroppedTransforms(plugin as { name?: string }, dropped, "mdast");
    };
    return result instanceof Promise ? result.then(apply) : apply(result);
  };

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    while (i < plugins.length) {
      const raw = plugins[i++]!;
      const plugin: MdastPluginDefinition = typeof raw === "function" ? raw() : raw;
      const r = runPlugin(plugin as MdastPluginInstance);
      if (r instanceof Promise) return r.then(runNext);
    }
    return { handle };
  };

  return runNext();
}

function runHastPluginsOnHandle(
  handle: HastHandle,
  plugins: HastPluginInput[],
  source: string,
  fileURL: URL | undefined,
  data: Data,
): void | Promise<void> {
  if (plugins.length === 0) return;

  let i = 0;
  const runNext = (): void | Promise<void> => {
    while (i < plugins.length) {
      const raw = plugins[i]!;
      i++;
      const plugin: HastPluginDefinition = typeof raw === "function" ? raw() : raw;

      const subs = resolveSubscriptions(plugin);
      const result = visitHastHandle(handle, plugin, subs, source, fileURL, data);
      const warnIfDropped = (dropped: number): void => {
        if (dropped) warnDroppedTransforms(plugin, dropped, "hast");
      };
      if (result instanceof Promise) {
        return result.then((dropped) => {
          warnIfDropped(dropped);
          return runNext();
        });
      }
      warnIfDropped(result);
    }
  };

  return runNext();
}

// Public API

function mdxOptionsToNative(opts: {
  optimizeStatic?: OptimizeStaticConfig;
  jsxImportSource?: string;
  jsx?: boolean;
  jsxRuntime?: "automatic" | "classic";
  development?: boolean;
  providerImportSource?: string;
  pragma?: string;
  pragmaFrag?: string;
  pragmaImportSource?: string;
  outputFormat?: "program" | "function-body";
  elementAttributeNameCase?: "react" | "html";
  stylePropertyNameCase?: "dom" | "css";
}) {
  const hasAny =
    opts.optimizeStatic ||
    opts.jsxImportSource !== undefined ||
    opts.jsx !== undefined ||
    opts.jsxRuntime !== undefined ||
    opts.development !== undefined ||
    opts.providerImportSource !== undefined ||
    opts.pragma !== undefined ||
    opts.pragmaFrag !== undefined ||
    opts.pragmaImportSource !== undefined ||
    opts.outputFormat !== undefined ||
    opts.elementAttributeNameCase !== undefined ||
    opts.stylePropertyNameCase !== undefined;
  if (!hasAny) return undefined;
  const result: Record<string, any> = {};
  if (opts.optimizeStatic) result.optimizeStatic = opts.optimizeStatic;
  if (opts.jsxImportSource !== undefined) result.jsxImportSource = opts.jsxImportSource;
  if (opts.jsx !== undefined) result.jsx = opts.jsx;
  if (opts.jsxRuntime !== undefined) result.jsxRuntime = opts.jsxRuntime;
  if (opts.development !== undefined) result.development = opts.development;
  if (opts.providerImportSource !== undefined)
    result.providerImportSource = opts.providerImportSource;
  if (opts.pragma !== undefined) result.pragma = opts.pragma;
  if (opts.pragmaFrag !== undefined) result.pragmaFrag = opts.pragmaFrag;
  if (opts.pragmaImportSource !== undefined) result.pragmaImportSource = opts.pragmaImportSource;
  if (opts.outputFormat !== undefined) result.outputFormat = opts.outputFormat;
  if (opts.elementAttributeNameCase !== undefined)
    result.elementAttributeNameCase = opts.elementAttributeNameCase;
  if (opts.stylePropertyNameCase !== undefined)
    result.stylePropertyNameCase = opts.stylePropertyNameCase;
  return result;
}

/** Configuration for static subtree collapsing during MDX compilation. */
export interface OptimizeStaticConfig {
  component: string;
  prop: string;
  wrapPropValue?: boolean;
  ignoreElements?: string[];
}

/** Granular smart-punctuation toggles. Omitted fields default to true. */
export interface SmartPunctuationOptions {
  /** Replace straight quotes with curly/smart quotes. Default: true. */
  quotes?: boolean;
  /** Replace `--`/`---` with en-dash/em-dash. Default: true. */
  dashes?: boolean;
  /** Replace `...` with ellipsis (`…`). Default: true. */
  ellipses?: boolean;
}

/**
 * Per-backref callback. Invoked once per anchor in the footnotes section
 * with 1-based `referenceNumber` and `rerunIndex` (1 on the first backref
 * to that definition, 2 on the second, and so on). Must return the final
 * string used as the backref content or `aria-label`.
 */
export type FootnoteBackrefCallback = (referenceNumber: number, rerunIndex: number) => string;

/**
 * i18n strings for the GFM footnotes section. Mirrors `footnoteLabel`,
 * `footnoteBackLabel`, and `footnoteBackContent` from remark-rehype.
 *
 * `backContent` and `backLabel` each accept either a template string with
 * the `{reference}` placeholder (substituted with `1` or `1-2` to match
 * remark-rehype's default suffix), or a callback receiving the raw
 * `(referenceNumber, rerunIndex)` pair.
 *
 * Passing this object enables footnotes. To turn them off, use
 * `gfm: { footnotes: false }`.
 */
export interface FootnoteOptions {
  /** `<h2>` label opening the footnotes section. Default: `"Footnotes"`. */
  label?: string;
  /**
   * Backref `<a>` content. Default: `"↩"`.
   *
   * Template form: the string is used as-is, and a `<sup>K</sup>` marker
   * is auto-appended on reruns (k > 1). Callback form: returns the full
   * content for each backref; no auto-sup is added.
   */
  backContent?: string | FootnoteBackrefCallback;
  /**
   * Backref `aria-label`. Default: `"Back to reference {reference}"`.
   *
   * Template form: `{reference}` becomes `n` for the first backref, `n-K`
   * for subsequent ones. Callback form: returns the `aria-label` string
   * for each backref.
   */
  backLabel?: string | FootnoteBackrefCallback;
}

/** Granular GFM toggles, nested under {@link Features.gfm}. */
export interface GfmOptions {
  /**
   * Enable GFM footnotes (`[^id]`). Default: true.
   *
   * Pass `false` to drop footnote parsing while keeping the rest of GFM;
   * pass an object to enable footnotes with custom i18n strings (see
   * {@link FootnoteOptions}).
   */
  footnotes?: boolean | FootnoteOptions;
}

/** Granular math toggles, nested under {@link Features.math}. */
export interface MathOptions {
  /**
   * Treat single-dollar runs (`$ ... $`) as inline math. Default: true.
   *
   * Set `false` to keep single `$` as literal text while still parsing
   * `$$ ... $$` display math. Mirrors `singleDollarTextMath` from
   * remark-math.
   */
  singleDollarTextMath?: boolean;
}

/** Parser feature toggles. All default to their documented value when omitted. */
export interface Features {
  /**
   * GFM: tables, footnotes, strikethrough, task lists. Default: true.
   *
   * Pass an options object for granular control:
   * ```ts
   * gfm: { footnotes: false }                   // skip footnotes only
   * gfm: { footnotes: { label: "Notes" } }      // localize footnotes
   * ```
   */
  gfm?: boolean | GfmOptions;
  /** Frontmatter: YAML (`--- ... ---`) and TOML (`+++ ... +++`). Default: true. */
  frontmatter?: boolean;
  /**
   * Math blocks and inline math. Default: false.
   *
   * Pass an options object for granular control:
   * ```ts
   * math: { singleDollarTextMath: false }       // $$..$$ only, $..$ literal
   * ```
   */
  math?: boolean | MathOptions;
  /** Heading attributes (`# text { #id .class }`). Default: false. */
  headingAttributes?: boolean;
  /** Colon-delimited container directive blocks (`:::`). Default: false. */
  directive?: boolean;
  /** Superscript (`^super^`). Default: false. */
  superscript?: boolean;
  /** Subscript (`~sub~`). Default: false. */
  subscript?: boolean;
  /** Obsidian-style wikilinks (`[[link]]`). Default: false. */
  wikilinks?: boolean;
  /**
   * Smart punctuation à la SmartyPants. Default: false.
   *
   * Pass `true` to enable all categories, or an options object for granular control:
   * ```ts
   * smartPunctuation: { dashes: false } // quotes + ellipses only
   * ```
   */
  smartPunctuation?: boolean | SmartPunctuationOptions;
}

export interface CompileOptions {
  mdastPlugins?: MdastPluginInput[];
  hastPlugins?: HastPluginInput[];
  features?: Features;
  /**
   * The document being processed, surfaced to plugins as `ctx.fileURL`. Must
   * be a `URL` (e.g. Astro's `fileURL`); convert a filesystem path with Node's
   * `pathToFileURL` before passing it.
   */
  fileURL?: URL;
  /**
   * Initial document-level data bag, seeding `ctx.data` before any plugin runs.
   * It is the same object plugins mutate and the caller reads back as
   * `result.data`, so values flow both into and out of a compile. Defaults to a
   * fresh empty object. Used by reference and mutated in place, so pass a
   * throwaway object per compile rather than a shared one.
   */
  data?: Data;
}

/**
 * MDX-only compile options.
 *
 * These are the fields specific to MDX compilation, separate from the shared
 * pipeline options in {@link CompileOptions}. Useful for wrappers (Vite/Rollup
 * plugins, framework integrations) that want to expose MDX-specific knobs
 * without re-exposing the shared pipeline fields.
 */
export interface MdxOnlyOptions {
  optimizeStatic?: OptimizeStaticConfig;
  /** Place to import automatic JSX runtimes from (e.g. "react", "preact"). Default: "react". */
  jsxImportSource?: string;
  /** Whether to keep JSX instead of compiling it to functions. Default: false. */
  jsx?: boolean;
  /** JSX runtime: "automatic" (default) or "classic". */
  jsxRuntime?: "automatic" | "classic";
  /** Enable development mode. Default: false. */
  development?: boolean;
  /** Place to import the component provider from. */
  providerImportSource?: string;
  /** Pragma for JSX in classic runtime (default: "React.createElement"). */
  pragma?: string;
  /** Pragma for JSX fragments in classic runtime (default: "React.Fragment"). */
  pragmaFrag?: string;
  /** Where to import the pragma from in classic runtime (default: "react"). */
  pragmaImportSource?: string;
  /**
   * Output format: "program" (default) or "function-body".
   *
   * - `"program"`: ES module with `import`/`export` statements.
   * - `"function-body"`: Function body that reads runtime from `arguments[0]`
   *   and returns `{ default: MDXContent, ...exports }`. Suitable for
   *   `new Function()` or `evaluate()`.
   */
  outputFormat?: "program" | "function-body";
  /**
   * Casing for HTML/SVG attribute names on plain (rehype-produced) elements.
   *
   * - `"react"` (default): `className`, `htmlFor`, `strokeLinecap`, `xmlLang`.
   * - `"html"`: `class`, `for`, `stroke-linecap`, `xml:lang`.
   *
   * Does not affect attributes on user-written MDX JSX; those are emitted as
   * the author wrote them.
   */
  elementAttributeNameCase?: "react" | "html";
  /**
   * Casing for keys in `style` objects parsed from `style="…"` strings on
   * plain (rehype-produced) elements.
   *
   * - `"dom"` (default): `{backgroundColor: …, WebkitLineClamp: …}`.
   * - `"css"`: `{"background-color": …, "-webkit-line-clamp": …}`.
   */
  stylePropertyNameCase?: "dom" | "css";
}

export interface MdxCompileOptions extends CompileOptions, MdxOnlyOptions {}

/** Frontmatter block extracted from the parsed Markdown/MDX source. */
export interface Frontmatter {
  /** Delimiter syntax used for the block. */
  kind: "yaml" | "toml";
  /** Raw content between the delimiters (`---`/`+++` lines excluded). */
  value: string;
}

/** Result of {@link markdownToHtml}. */
export interface MarkdownToHtmlResult {
  /** Rendered HTML string. */
  html: string;
  /** Frontmatter block at the start of the document, or `null` if none. */
  frontmatter: Frontmatter | null;
  /** Document-level data bag shared with plugins via `ctx.data`; the seeded `data` option if provided, else a fresh `{}`. */
  data: Data;
}

/** Result of {@link mdxToJs}. */
export interface MdxToJsResult {
  /** Compiled JavaScript module source. */
  code: string;
  /** Frontmatter block at the start of the document, or `null` if none. */
  frontmatter: Frontmatter | null;
  /** Document-level data bag shared with plugins via `ctx.data`; the seeded `data` option if provided, else a fresh `{}`. */
  data: Data;
}

// Type helpers: detect whether any visitor in any plugin returns a Promise.
// Used to narrow `markdownToHtml`/`mdxToJs` to a sync return when every plugin
// is sync, while keeping the union when at least one visitor is async.

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFn = (...args: any[]) => unknown;
type ReturnsPromise<F> = F extends AnyFn
  ? Extract<ReturnType<F>, Promise<unknown>> extends never
    ? false
    : true
  : false;
type FieldIsAsync<V> = V extends AnyFn
  ? ReturnsPromise<V>
  : V extends { visit: infer F }
    ? ReturnsPromise<F>
    : V extends ReadonlyArray<infer Item>
      ? Item extends { visit: infer F }
        ? ReturnsPromise<F>
        : false
      : false;
type AnyVisitorAsync<P> = {
  [K in keyof P]-?: FieldIsAsync<NonNullable<P[K]>>;
}[keyof P];
type IsPluginAsync<P> = true extends AnyVisitorAsync<P> ? true : false;
type ResolveInput<P> = P extends () => infer D ? D : P;
type AnyInputAsync<Ps> =
  Ps extends ReadonlyArray<infer P>
    ? true extends IsPluginAsync<ResolveInput<P>>
      ? true
      : false
    : false;
type OptionsAsync<O> = (
  O extends { mdastPlugins: infer Ps } ? AnyInputAsync<Ps> : false
) extends true
  ? true
  : (O extends { hastPlugins: infer Ps } ? AnyInputAsync<Ps> : false) extends true
    ? true
    : false;

type ResultFor<O, R> = OptionsAsync<O> extends true ? Promise<R> : R;

export function markdownToHtml<O extends CompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MarkdownToHtmlResult>;
export function markdownToHtml(
  source: string,
  options: CompileOptions = {},
): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> {
  const { mdastPlugins = [], hastPlugins = [], features, fileURL, data = {} } = options;
  const hastMayHaveStubs = hastPlugins.length > 0;
  const { features: nativeFeatures, convertOptions: nativeConvertOptions } =
    featuresToNative(features);

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    false,
    fileURL,
    nativeFeatures,
    nativeConvertOptions,
    data,
  );

  const renderAndDrop = (h: HastHandle, frontmatter: Frontmatter | null): MarkdownToHtmlResult => {
    try {
      const html = renderHandle(h);
      return { html, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
  };

  const runHastThenRender = (
    r: HastWithFrontmatter,
  ): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> => {
    let hastResult: void | Promise<void>;
    try {
      hastResult = runHastPluginsOnHandle(r.hastHandle, hastPlugins, source, fileURL, data);
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (hastResult instanceof Promise) {
      return hastResult.then(
        () => renderAndDrop(r.hastHandle, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return renderAndDrop(r.hastHandle, r.frontmatter);
  };

  if (result instanceof Promise) return result.then(runHastThenRender);
  return runHastThenRender(result);
}

export function mdxToJs<O extends MdxCompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MdxToJsResult>;
export function mdxToJs(
  source: string,
  options: MdxCompileOptions = {},
): MdxToJsResult | Promise<MdxToJsResult> {
  const {
    mdastPlugins = [],
    hastPlugins = [],
    features,
    fileURL,
    data = {},
    ...mdxFields
  } = options;
  const hastMayHaveStubs = hastPlugins.length > 0;
  const mdxOptions = mdxOptionsToNative(mdxFields);
  const { features: nativeFeatures, convertOptions: nativeConvertOptions } =
    featuresToNative(features);

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    true,
    fileURL,
    nativeFeatures,
    nativeConvertOptions,
    data,
  );

  const compileAndDrop = (h: HastHandle, frontmatter: Frontmatter | null): MdxToJsResult => {
    try {
      const code = compileHandle(h, mdxOptions);
      return { code, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
  };

  const runHastThenCompile = (r: HastWithFrontmatter): MdxToJsResult | Promise<MdxToJsResult> => {
    let hastResult: void | Promise<void>;
    try {
      hastResult = runHastPluginsOnHandle(r.hastHandle, hastPlugins, source, fileURL, data);
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (hastResult instanceof Promise) {
      return hastResult.then(
        () => compileAndDrop(r.hastHandle, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return compileAndDrop(r.hastHandle, r.frontmatter);
  };

  if (result instanceof Promise) return result.then(runHastThenCompile);
  return runHastThenCompile(result);
}

export interface EvaluateOptions extends Omit<MdxCompileOptions, "jsx" | "outputFormat"> {
  Fragment: unknown;
  jsx: (type: unknown, props: unknown, key?: unknown) => unknown;
  jsxs: (type: unknown, props: unknown, key?: unknown) => unknown;
  jsxDEV?: (
    type: unknown,
    props: unknown,
    key: unknown,
    isStaticChildren: boolean,
    source: unknown,
    self: unknown,
  ) => unknown;
  useMDXComponents?: () => Record<string, unknown>;
}

/**
 * Compile and evaluate MDX in one step.
 *
 * Returns the module's exports, including `default` (the MDX component).
 * Returns a Promise when async plugins are used, otherwise returns synchronously.
 *
 * ```ts
 * import * as runtime from "react/jsx-runtime";
 * const { default: Content } = evaluate("# Hello", { ...runtime });
 * ```
 */
export function evaluate(
  source: string,
  options: EvaluateOptions,
): Record<string, unknown> | Promise<Record<string, unknown>> {
  const { Fragment, jsx, jsxs, jsxDEV, useMDXComponents, ...compileOpts } = options;
  const runtime = { Fragment, jsx, jsxs, jsxDEV, useMDXComponents };
  const result = mdxToJs(source, { ...compileOpts, outputFormat: "function-body" });
  if (result instanceof Promise) {
    return result.then((resolved) => new Function(resolved.code)(runtime));
  }
  return new Function(result.code)(runtime);
}

// Pipeline: parse → mdast plugins → hast conversion → hast plugins
// All arenas stay in Rust. No intermediate buffer copies to JS.

type HastWithFrontmatter = { hastHandle: HastHandle; frontmatter: Frontmatter | null };

function readFrontmatter(handle: MdastHandle): Frontmatter | null {
  const raw = getMdastFrontmatter(handle);
  return raw ? { kind: raw.kind === "toml" ? "toml" : "yaml", value: raw.value } : null;
}

/** Parse, run mdast plugins, capture frontmatter, then convert to HAST.
 *  Frontmatter is read from the post-plugin MDAST so visitor mutations to
 *  the yaml/toml node are reflected in the returned value. */
function createHastHandleFromMdast(
  source: string,
  mdastPlugins: MdastPluginInput[],
  mdx: boolean,
  fileURL: URL | undefined,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  nativeFeatures: any,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  nativeConvertOptions: any,
  data: Data,
): HastWithFrontmatter | Promise<HastWithFrontmatter> {
  const mdastHandle = mdx
    ? createMdxMdastHandle(source, nativeFeatures)
    : createMdastHandle(source, nativeFeatures);

  const mdastMayHaveStubs = mdastPlugins.length > 0;

  // finally{release} is intentional: convertMdastToHastHandle empties the arena
  // on success, but if any step here throws the handle would otherwise leak.
  const finalize = (r: MdastPipelineResult): HastWithFrontmatter => {
    try {
      const frontmatter = readFrontmatter(r.handle);
      // convert empties the mdast arena, so invalidate any stub held past this point.
      if (mdastMayHaveStubs) markHandleMutated(r.handle);
      const hastHandle = convertMdastToHastHandle(r.handle, nativeConvertOptions);
      return { hastHandle, frontmatter };
    } finally {
      releaseHandle(r.handle, mdastMayHaveStubs);
    }
  };

  try {
    if (mdastPlugins.length === 0) {
      return finalize({ handle: mdastHandle });
    }

    const mdastResult = runMdastPluginsOnHandle(mdastHandle, mdastPlugins, fileURL, data);

    if (mdastResult instanceof Promise) {
      return mdastResult.then(finalize, (err) => {
        releaseHandle(mdastHandle, mdastMayHaveStubs);
        throw err;
      });
    }
    return finalize(mdastResult);
  } catch (err) {
    releaseHandle(mdastHandle, mdastMayHaveStubs);
    throw err;
  }
}

// Step-by-step API: individual pipeline stages with materialized trees

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdastHandle(source, featuresToNative(options.features).features);
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdxMdastHandle(source, featuresToNative(options.features).features);
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string, options: { features?: Features } = {}): HastNode {
  const { features: nativeFeatures, convertOptions } = featuresToNative(options.features);
  const handle = createHastHandle(source, nativeFeatures, convertOptions);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string, options: { features?: Features } = {}): HastNode {
  const { features: nativeFeatures, convertOptions } = featuresToNative(options.features);
  const handle = createMdxHastHandle(source, nativeFeatures, convertOptions);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}
