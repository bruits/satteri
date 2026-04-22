import { visitHastHandle, resolveSubscriptions, type HastHandle } from "./hast/hast-visitor.js";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastPluginInstance,
} from "./mdast/mdast-visitor.js";
import type { MdastPluginDefinition, HastPluginDefinition } from "./plugin.js";
import {
  parseToHtml,
  compileMdx,
  createHastHandle,
  createMdxHastHandle,
  renderHandle,
  compileHandle,
  applyCommandsToHandle,
  dropHandle,
  createMdastHandle,
  createMdxMdastHandle,
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  applyCommandsAndConvertToHastHandle,
  getHandleSource,
  serializeHandle,
  serializeMdastHandle,
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
  plugins: MdastPluginDefinition[],
  filename: string,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  let pendingCommands: Uint8Array | null = null;
  const source = getHandleSource(handle);

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    while (i < plugins.length) {
      const idx = i++;
      const plugin = plugins[idx]!;
      const subs = resolveMdastSubscriptions(plugin as MdastPluginInstance);
      const result = visitMdastHandle(
        handle,
        plugin as MdastPluginInstance,
        subs,
        source,
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
  plugins: HastPluginDefinition[],
  source: string,
  filename: string,
): void | Promise<void> {
  if (plugins.length === 0) return;

  let i = 0;
  const runNext = (): void | Promise<void> => {
    while (i < plugins.length) {
      const plugin = plugins[i]!;
      i++;

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
    opts.outputFormat !== undefined;
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
  mdastPlugins?: MdastPluginDefinition[];
  hastPlugins?: HastPluginDefinition[];
  features?: Features;
  filename?: string;
}

export interface MdxCompileOptions extends CompileOptions {
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
}

export function markdownToHtml(
  source: string,
  options: CompileOptions = {},
): string | Promise<string> {
  const { mdastPlugins = [], hastPlugins = [], features, filename = "<unknown>" } = options;
  const nativeFeatures = featuresToNative(features);

  // TODO: When there's no plugins, we shouldn't go through all the steps below, we could just call parseToHtml directly.
  // However right now pulldown-cmark's HTML output is super different from our target (unified's). So until that's fixed, we'll do the slower pipeline

  const handleResult = createHastHandleFromMdast(
    source,
    mdastPlugins,
    false,
    filename,
    nativeFeatures,
  );

  const finish = (hastHandle: HastHandle): string | Promise<string> => {
    const asyncResult = runHastPluginsOnHandle(hastHandle, hastPlugins, source, filename);
    if (asyncResult instanceof Promise) {
      return asyncResult.then(() => {
        const html = renderHandle(hastHandle);
        dropHandle(hastHandle);
        return html;
      });
    }
    const html = renderHandle(hastHandle);
    dropHandle(hastHandle);
    return html;
  };

  if (handleResult instanceof Promise) {
    return handleResult.then(finish);
  }
  return finish(handleResult);
}

export function mdxToJs(source: string, options: MdxCompileOptions = {}): string | Promise<string> {
  const {
    mdastPlugins = [],
    hastPlugins = [],
    features,
    filename = "<unknown>",
    ...mdxFields
  } = options;
  const mdxOptions = mdxOptionsToNative(mdxFields);
  const nativeFeatures = featuresToNative(features);

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    return compileMdx(source, mdxOptions, nativeFeatures);
  }

  const handleResult = createHastHandleFromMdast(
    source,
    mdastPlugins,
    true,
    filename,
    nativeFeatures,
  );

  const finish = (hastHandle: HastHandle): string | Promise<string> => {
    const asyncResult = runHastPluginsOnHandle(hastHandle, hastPlugins, source, filename);
    if (asyncResult instanceof Promise) {
      return asyncResult.then(() => {
        const js = compileHandle(hastHandle, mdxOptions);
        dropHandle(hastHandle);
        return js;
      });
    }
    const js = compileHandle(hastHandle, mdxOptions);
    dropHandle(hastHandle);
    return js;
  };

  if (handleResult instanceof Promise) {
    return handleResult.then(finish);
  }
  return finish(handleResult);
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
  const code = mdxToJs(source, { ...compileOpts, outputFormat: "function-body" });
  if (code instanceof Promise) {
    return code.then((resolved) => new Function(resolved)(runtime));
  }
  return new Function(code)(runtime);
}

// Pipeline: parse → mdast plugins → hast conversion → hast plugins
// All arenas stay in Rust. No intermediate buffer copies to JS.

/** Parse + mdast plugins + convert to HAST handle. */
function createHastHandleFromMdast(
  source: string,
  mdastPlugins: MdastPluginDefinition[],
  mdx: boolean,
  filename: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  nativeFeatures?: any,
): HastHandle | Promise<HastHandle> {
  if (mdastPlugins.length === 0) {
    return mdx
      ? createMdxHastHandle(source, nativeFeatures)
      : createHastHandle(source, nativeFeatures);
  }

  const mdastHandle = mdx
    ? createMdxMdastHandle(source, nativeFeatures)
    : createMdastHandle(source, nativeFeatures);
  const mdastResult = runMdastPluginsOnHandle(mdastHandle, mdastPlugins, filename);

  const convert = (r: MdastPipelineResult): HastHandle => {
    if (r.pendingCommands) {
      return applyCommandsAndConvertToHastHandle(r.handle, r.pendingCommands);
    }
    return convertMdastToHastHandle(r.handle);
  };

  if (mdastResult instanceof Promise) {
    return mdastResult.then(convert);
  }
  return convert(mdastResult);
}

// Step-by-step API: individual pipeline stages with materialized trees

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdastHandle(source, featuresToNative(options.features));
  const buf = serializeMdastHandle(handle);
  return materializeMdastTree(new MdastReader(buf));
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string, options: { features?: Features } = {}): MdastNode {
  const handle = createMdxMdastHandle(source, featuresToNative(options.features));
  const buf = serializeMdastHandle(handle);
  return materializeMdastTree(new MdastReader(buf));
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string, options: { features?: Features } = {}): HastNode {
  const handle = createHastHandle(source, featuresToNative(options.features));
  const buf = serializeHandle(handle);
  dropHandle(handle);
  return materializeHastTree(new HastReader(buf));
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string, options: { features?: Features } = {}): HastNode {
  const handle = createMdxHastHandle(source, featuresToNative(options.features));
  const buf = serializeHandle(handle);
  dropHandle(handle);
  return materializeHastTree(new HastReader(buf));
}
