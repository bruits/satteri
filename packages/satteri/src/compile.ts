import { visitHastHandle, resolveSubscriptions, type HastHandle } from "./hast/hast-visitor.js";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
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
import { HastReader } from "./hast/hast-reader.js";
import { materializeHastTree } from "./hast/hast-materializer.js";
import type { MdastNode, HastNode } from "./types.js";

function featuresToNative(features: Features | undefined) {
  if (!features) return undefined;
  const result: Record<string, unknown> = {};
  if (features.gfm !== undefined) result.gfm = features.gfm;
  if (features.frontmatter !== undefined) result.frontmatter = features.frontmatter;
  if (features.math !== undefined) result.math = features.math;
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
  return result;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type MdastHandle = any;

type MdastPipelineResult = { handle: MdastHandle; pendingCommands: Uint8Array | null };

function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginInput[],
  filename: string,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  let pendingCommands: Uint8Array | null = null;

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    while (i < plugins.length) {
      const idx = i++;
      const raw = plugins[idx]!;
      const plugin: MdastPluginDefinition = typeof raw === "function" ? raw() : raw;
      const subs = resolveMdastSubscriptions(plugin as MdastPluginInstance);
      const result = visitMdastHandle(
        handle,
        plugin as MdastPluginInstance,
        subs,
        () => getHandleSource(handle),
        filename,
      );

      if (result instanceof Promise) {
        return result.then((r) => {
          applyMdastResult(r, idx, plugins.length, handle);
          return runNext();
        });
      }

      applyMdastResult(result, idx, plugins.length, handle);
    }
    return { handle, pendingCommands };
  };

  function applyMdastResult(
    result: { commandBuffer: Uint8Array; hasMutations: boolean },
    idx: number,
    total: number,
    h: MdastHandle,
  ) {
    if (result.hasMutations) {
      if (idx === total - 1) {
        pendingCommands = result.commandBuffer;
      } else {
        applyCommandsToMdastHandle(h, result.commandBuffer);
      }
    }
  }

  return runNext();
}

function runHastPluginsOnHandle(
  handle: HastHandle,
  plugins: HastPluginInput[],
  source: string,
  filename: string,
): void | Promise<void> {
  if (plugins.length === 0) return;

  let i = 0;
  const runNext = (): void | Promise<void> => {
    while (i < plugins.length) {
      const raw = plugins[i]!;
      i++;
      const plugin: HastPluginDefinition = typeof raw === "function" ? raw() : raw;

      const subs = resolveSubscriptions(plugin);
      const result = visitHastHandle(handle, plugin, subs, source, filename);
      if (result instanceof Promise) {
        return result.then(runNext);
      }
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

/** Parser feature toggles. All default to their documented value when omitted. */
export interface Features {
  /** GFM: tables, footnotes, strikethrough, task lists. Default: true. */
  gfm?: boolean;
  /** Frontmatter: YAML (`--- ... ---`) and TOML (`+++ ... +++`). Default: true. */
  frontmatter?: boolean;
  /** Math blocks and inline math. Default: true. */
  math?: boolean;
  /** Heading attributes (`# text { #id .class }`). Default: true. */
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
  filename?: string;
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
}

/** Result of {@link mdxToJs}. */
export interface MdxToJsResult {
  /** Compiled JavaScript module source. */
  code: string;
  /** Frontmatter block at the start of the document, or `null` if none. */
  frontmatter: Frontmatter | null;
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
  const { mdastPlugins = [], hastPlugins = [], features, filename = "<unknown>" } = options;
  const nativeFeatures = featuresToNative(features);

  const result = createHastHandleFromMdast(source, mdastPlugins, false, filename, nativeFeatures);

  const renderAndDrop = (h: HastHandle, frontmatter: Frontmatter | null): MarkdownToHtmlResult => {
    try {
      const html = renderHandle(h);
      return { html, frontmatter };
    } finally {
      dropHandle(h);
    }
  };

  const runHastThenRender = (
    r: HastWithFrontmatter,
  ): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> => {
    let hastResult: void | Promise<void>;
    try {
      hastResult = runHastPluginsOnHandle(r.hastHandle, hastPlugins, source, filename);
    } catch (err) {
      dropHandle(r.hastHandle);
      throw err;
    }
    if (hastResult instanceof Promise) {
      return hastResult.then(
        () => renderAndDrop(r.hastHandle, r.frontmatter),
        (err) => {
          dropHandle(r.hastHandle);
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
    filename = "<unknown>",
    ...mdxFields
  } = options;
  const mdxOptions = mdxOptionsToNative(mdxFields);
  const nativeFeatures = featuresToNative(features);

  const result = createHastHandleFromMdast(source, mdastPlugins, true, filename, nativeFeatures);

  const compileAndDrop = (h: HastHandle, frontmatter: Frontmatter | null): MdxToJsResult => {
    try {
      const code = compileHandle(h, mdxOptions);
      return { code, frontmatter };
    } finally {
      dropHandle(h);
    }
  };

  const runHastThenCompile = (r: HastWithFrontmatter): MdxToJsResult | Promise<MdxToJsResult> => {
    let hastResult: void | Promise<void>;
    try {
      hastResult = runHastPluginsOnHandle(r.hastHandle, hastPlugins, source, filename);
    } catch (err) {
      dropHandle(r.hastHandle);
      throw err;
    }
    if (hastResult instanceof Promise) {
      return hastResult.then(
        () => compileAndDrop(r.hastHandle, r.frontmatter),
        (err) => {
          dropHandle(r.hastHandle);
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
  filename: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  nativeFeatures?: any,
): HastWithFrontmatter | Promise<HastWithFrontmatter> {
  const mdastHandle = mdx
    ? createMdxMdastHandle(source, nativeFeatures)
    : createMdastHandle(source, nativeFeatures);

  // finally{drop} is intentional: convertMdastToHastHandle empties the arena
  // on success, but if any step here throws the handle would otherwise leak.
  const finalize = (r: MdastPipelineResult): HastWithFrontmatter => {
    try {
      if (r.pendingCommands) {
        applyCommandsToMdastHandle(r.handle, r.pendingCommands);
      }
      const frontmatter = readFrontmatter(r.handle);
      const hastHandle = convertMdastToHastHandle(r.handle);
      return { hastHandle, frontmatter };
    } finally {
      dropHandle(r.handle);
    }
  };

  try {
    if (mdastPlugins.length === 0) {
      return finalize({ handle: mdastHandle, pendingCommands: null });
    }

    const mdastResult = runMdastPluginsOnHandle(mdastHandle, mdastPlugins, filename);

    if (mdastResult instanceof Promise) {
      return mdastResult.then(finalize, (err) => {
        dropHandle(mdastHandle);
        throw err;
      });
    }
    return finalize(mdastResult);
  } catch (err) {
    dropHandle(mdastHandle);
    throw err;
  }
}

// Step-by-step API: individual pipeline stages with materialized trees

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdastHandle(source, featuresToNative(options.features));
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    dropHandle(handle);
  }
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdxMdastHandle(source, featuresToNative(options.features));
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    dropHandle(handle);
  }
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string, options: { features?: Features } = {}): HastNode {
  const handle = createHastHandle(source, featuresToNative(options.features));
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    dropHandle(handle);
  }
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string, options: { features?: Features } = {}): HastNode {
  const handle = createMdxHastHandle(source, featuresToNative(options.features));
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    dropHandle(handle);
  }
}
