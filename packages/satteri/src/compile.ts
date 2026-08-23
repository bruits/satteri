import {
  visitHastHandle,
  visitHastHandleCollect,
  visitHastHook,
  visitHastHookCollect,
  resolveSubscriptions,
  type HastDiagnostic,
  type HastHandle,
  type HastHookFn,
} from "./hast/hast-visitor.js";
import {
  visitMdastHandle,
  visitMdastHook,
  resolveMdastSubscriptions,
  type MdastDiagnostic,
  type MdastHandle,
  type MdastHookFn,
  type MdastPluginInstance,
} from "./mdast/mdast-visitor.js";
import { normalizePlugins } from "./plugin.js";
import type {
  MdastPluginDefinition,
  HastPluginDefinition,
  MdastPluginList,
  HastPluginList,
} from "./plugin.js";
import {
  applyCommandsAndCompileHandle,
  applyCommandsAndRenderHandle,
  applyCommandsToMdastHandle,
  applyMdastCommandsAndConvertAndCompile,
  applyMdastCommandsAndConvertAndRender,
  compileHandle,
  convertMdastToHastHandle,
  createHastHandle,
  createHastHandleFromHtml,
  createMdastHandle,
  createMdxHastHandle,
  createMdxMdastHandle,
  dropHandle,
  getHandleSource,
  createHastHandleWithFrontmatter,
  createMdxHastHandleWithFrontmatter,
  getMdastFrontmatter,
  markdownToHtmlFast,
  markdownToJsFast,
  mdxToJsFast,
  renderHandle,
  serializeHandle,
} from "#binding";
import {
  ENABLE_DEFINITION_LIST,
  ENABLE_DIRECTIVE,
  ENABLE_FOOTNOTES,
  ENABLE_GFM,
  ENABLE_HEADING_ATTRIBUTES,
  ENABLE_MATH,
  ENABLE_MATH_MULTI_DOLLAR,
  ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
  ENABLE_SMART_DASHES,
  ENABLE_SMART_ELLIPSES,
  ENABLE_SMART_PUNCTUATION,
  ENABLE_SMART_QUOTES,
  ENABLE_STRIKETHROUGH,
  ENABLE_SUBSCRIPT,
  ENABLE_SUPERSCRIPT,
  ENABLE_TABLES,
  ENABLE_TASKLISTS,
  ENABLE_WIKILINKS,
  ENABLE_YAML_STYLE_METADATA_BLOCKS,
} from "./generated/parse-options.js";
import { MdastReader } from "./mdast/mdast-reader.js";
import { materializeMdastTree } from "./mdast/mdast-materializer.js";
import { markHandleMutated } from "./lazy-child-resolver.js";
import { HastReader } from "./hast/hast-reader.js";
import { materializeHastTree } from "./hast/hast-materializer.js";
import type { MdastNode, HastNode, Data, SourceFormat } from "./types.js";

const GFM_PARSE_OPTIONS = ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS | ENABLE_GFM;
const FRONTMATTER_PARSE_OPTIONS =
  ENABLE_YAML_STYLE_METADATA_BLOCKS | ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;

export const DEFAULT_PARSE_OPTIONS =
  GFM_PARSE_OPTIONS | ENABLE_FOOTNOTES | FRONTMATTER_PARSE_OPTIONS;

type NativeConvertOptions = NonNullable<Parameters<typeof markdownToHtmlFast>[2]>;

type NativeCompileOptions = {
  parseOptions: number;
  convertOptions: NativeConvertOptions | undefined;
};

/** Packs into the bit set napi reads straight back as `satteri_pulldown_cmark::Options`. */
function featuresToParseOptions(features: Features): number {
  let options = 0;

  const { gfm } = features;
  if (typeof gfm === "object") {
    options |= GFM_PARSE_OPTIONS;
    if (gfm.footnotes !== false) options |= ENABLE_FOOTNOTES;
  } else if (gfm ?? true) {
    options |= GFM_PARSE_OPTIONS | ENABLE_FOOTNOTES;
  }

  if (features.frontmatter ?? true) options |= FRONTMATTER_PARSE_OPTIONS;

  // Opting out of single-dollar math sets the multi-dollar sub-flag so the parser skips lone `$`.
  const { math } = features;
  if (typeof math === "object") {
    options |= math.singleDollarTextMath === false ? ENABLE_MATH_MULTI_DOLLAR : ENABLE_MATH;
  } else if (math) {
    options |= ENABLE_MATH;
  }

  if (features.headingAttributes) options |= ENABLE_HEADING_ATTRIBUTES;
  if (features.directive) options |= ENABLE_DIRECTIVE;
  if (features.superscript) options |= ENABLE_SUPERSCRIPT;
  if (features.subscript) options |= ENABLE_SUBSCRIPT;
  if (features.wikilinks) options |= ENABLE_WIKILINKS;
  if (features.definitionList) options |= ENABLE_DEFINITION_LIST;

  const smartPunctuation = features.smartPunctuation;
  if (typeof smartPunctuation === "object") {
    if (smartPunctuation.quotes ?? true) options |= ENABLE_SMART_QUOTES;
    if (smartPunctuation.dashes ?? true) options |= ENABLE_SMART_DASHES;
    if (smartPunctuation.ellipses ?? true) options |= ENABLE_SMART_ELLIPSES;
  } else if (smartPunctuation) {
    options |= ENABLE_SMART_PUNCTUATION;
  }

  return options;
}

/**
 * Split the user-facing `Features` into the packed parser options plus the
 * conversion-side `JsConvertOptions` carrying the footnote i18n strings and the
 * raw-HTML reparse. The public API only exposes `features`; both halves are
 * routed to napi internally.
 */
export function featuresToNative(features: Features | undefined): NativeCompileOptions {
  if (!features) return { parseOptions: DEFAULT_PARSE_OPTIONS, convertOptions: undefined };
  let convertOptions: NativeConvertOptions | undefined;

  const { gfm } = features;
  if (typeof gfm === "object" && typeof gfm.footnotes === "object") {
    const { label, backContent, backLabel, clobberPrefix } = gfm.footnotes;
    convertOptions = {};
    if (label !== undefined) convertOptions.footnoteLabel = label;
    if (backContent !== undefined) convertOptions.footnoteBackContent = backContent;
    if (backLabel !== undefined) convertOptions.footnoteBackLabel = backLabel;
    if (clobberPrefix !== undefined) convertOptions.clobberPrefix = clobberPrefix;
  }
  if (features.rawHtml !== undefined) {
    // Routed to convert options so every pipeline (fast paths, plugin paths,
    // *ToHast) applies the reparse.
    convertOptions = convertOptions ?? {};
    convertOptions.rawHtml = features.rawHtml;
  }
  return { parseOptions: featuresToParseOptions(features), convertOptions };
}

type MdastPipelineResult = {
  handle: MdastHandle;
  /** The final plugin's unapplied commands when `collectLast` was requested
   *  and that plugin queued mutations; the caller fuses the apply with the
   *  downstream NAPI step. */
  pendingCommands?: Uint8Array;
  /** The plugin whose commands were deferred, for dropped-transform warning
   *  attribution after the fused apply. */
  lastPlugin?: { name?: string };
};

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
    `satteri: plugin "${name}" queued ${dropped} ${kind} ${noun} on node(s) that a plugin had ` +
      `already removed or replaced; ${dropped === 1 ? "it was" : "they were"} dropped.`,
  );
}

function settle<T>(value: T | Promise<T>, fn: (value: T) => void): Promise<void> | undefined {
  if (value instanceof Promise) return value.then(fn);
  fn(value);
  return undefined;
}

/** Stays synchronous until a step actually returns a promise. */
function sequence(
  steps: ((() => Promise<void> | undefined) | undefined)[],
): Promise<void> | undefined {
  let pending: Promise<void> | undefined;
  for (const step of steps) {
    if (step === undefined) continue;
    pending = pending === undefined ? step() : pending.then(step);
  }
  return pending;
}

function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginDefinition[],
  fileURL: URL | undefined,
  data: Data,
  sourceFormat: SourceFormat,
  collectLast = false,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  const out: MdastPipelineResult = { handle };

  // A plugin's visitors run once over the tree. A transform that passes a child
  // through (returning it inside the replacement) keeps that child's identity,
  // so a patch the same pass queued on it still applies: nesting composes in
  // one pass. Visitor-built nodes are not re-walked; transform them up front,
  // or hand off to a later plugin that sees the materialized tree.
  const runPlugin = (plugin: MdastPluginInstance, isLastPlugin: boolean): void | Promise<void> => {
    const subs = resolveMdastSubscriptions(plugin);
    const apply = (
      r: { commandBuffer: Uint8Array; hasMutations: boolean },
      isFinalPass: boolean,
    ): void => {
      if (!r.hasMutations) return;
      if (collectLast && isLastPlugin && isFinalPass) {
        out.pendingCommands = r.commandBuffer;
        out.lastPlugin = plugin as { name?: string };
        return;
      }
      markHandleMutated(handle);
      const dropped = applyCommandsToMdastHandle(handle, r.commandBuffer);
      if (dropped) warnDroppedTransforms(plugin as { name?: string }, dropped, "mdast");
    };

    const before = typeof plugin.before === "function" ? plugin.before : undefined;
    const after = typeof plugin.after === "function" ? plugin.after : undefined;
    const hasVisitors = subs.length > 0;
    // Threaded like `data`: one context per pass, since a resolver is epoch-bound.
    const diagnostics: MdastDiagnostic[] = [];
    const runHook = (hook: MdastHookFn, isFinalPass: boolean) => () =>
      settle(
        visitMdastHook(
          handle,
          plugin,
          hook,
          () => getHandleSource(handle),
          fileURL,
          data,
          sourceFormat,
          diagnostics,
        ),
        (r) => apply(r, isFinalPass),
      );
    const runVisitors = (isFinalPass: boolean) => () =>
      settle(
        visitMdastHandle(
          handle,
          plugin,
          subs,
          () => getHandleSource(handle),
          fileURL,
          data,
          sourceFormat,
          diagnostics,
        ),
        (r) => apply(r, isFinalPass),
      );

    return sequence([
      before ? runHook(before, !hasVisitors && after === undefined) : undefined,
      hasVisitors ? runVisitors(after === undefined) : undefined,
      after ? runHook(after, true) : undefined,
    ]);
  };

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    for (;;) {
      const plugin = plugins[i];
      if (plugin === undefined) break;
      i++;
      const r = runPlugin(plugin as MdastPluginInstance, i === plugins.length);
      if (r instanceof Promise) return r.then(runNext);
    }
    return out;
  };

  return runNext();
}

const EMPTY_COMMAND_BUFFER = new Uint8Array(0);

type CollectedHastCommands = {
  commands: Uint8Array;
  /** The plugin whose commands were collected, for dropped-transform warning
   *  attribution after the fused apply. */
  lastPlugin: { name?: string } | null;
};

const NO_HAST_COMMANDS: CollectedHastCommands = {
  commands: EMPTY_COMMAND_BUFFER,
  lastPlugin: null,
};

/** Run every HAST plugin except the last one normally (each applies its
 *  commands inline), then return the last plugin's commands without applying.
 *  Lets the caller fuse the final apply with the downstream NAPI step
 *  (`render` or `compile`), saving one NAPI roundtrip per compile. */
function runHastPluginsCollectLast(
  handle: HastHandle,
  plugins: HastPluginDefinition[],
  source: string,
  fileURL: URL | undefined,
  data: Data,
  sourceFormat: SourceFormat,
): CollectedHastCommands | Promise<CollectedHastCommands> {
  let i = 0;
  const runNext = (): CollectedHastCommands | Promise<CollectedHastCommands> => {
    for (;;) {
      const plugin = plugins[i];
      if (plugin === undefined) break;
      const isLastPlugin = i === plugins.length - 1;
      i++;
      const subs = resolveSubscriptions(plugin);
      const { before, after } = plugin;

      const passes: (HastHookFn | "visitors")[] = [];
      if (typeof before === "function") passes.push(before);
      if (subs.length > 0) passes.push("visitors");
      if (typeof after === "function") passes.push(after);
      const finalPass = passes.pop();
      if (finalPass === undefined) continue;

      const warnIfDropped = (dropped: number): void => {
        if (dropped) warnDroppedTransforms(plugin, dropped, "hast");
      };
      // Threaded like `data`: one context per pass, since a resolver is epoch-bound.
      const diagnostics: HastDiagnostic[] = [];
      const runApplied = (pass: HastHookFn | "visitors") => () =>
        settle(
          pass === "visitors"
            ? visitHastHandle(
                handle,
                plugin,
                subs,
                source,
                fileURL,
                data,
                sourceFormat,
                diagnostics,
              )
            : visitHastHook(handle, plugin, pass, source, fileURL, data, sourceFormat, diagnostics),
          warnIfDropped,
        );
      const collectFinal = (): CollectedHastCommands | Promise<CollectedHastCommands> => {
        const collected =
          finalPass === "visitors"
            ? visitHastHandleCollect(
                handle,
                plugin,
                subs,
                source,
                fileURL,
                data,
                sourceFormat,
                diagnostics,
              )
            : visitHastHookCollect(
                handle,
                plugin,
                finalPass,
                source,
                fileURL,
                data,
                sourceFormat,
                diagnostics,
              );
        return collected instanceof Promise
          ? collected.then((commands) => ({ commands, lastPlugin: plugin }))
          : { commands: collected, lastPlugin: plugin };
      };

      const head = sequence(passes.map(runApplied));

      if (isLastPlugin) {
        return head instanceof Promise ? head.then(collectFinal) : collectFinal();
      }

      const tail =
        head instanceof Promise ? head.then(runApplied(finalPass)) : runApplied(finalPass)();
      if (tail instanceof Promise) return tail.then(runNext);
    }
    return NO_HAST_COMMANDS;
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
  /** Prefix applied to footnote IDs to prevent DOM clobbering. Default: `"user-content-"`. */
  clobberPrefix?: string;
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
  /** Definition lists (a term line then a `: definition`). Default: false. */
  definitionList?: boolean;
  /**
   * Smart punctuation à la SmartyPants. Default: false.
   *
   * Pass `true` to enable all categories, or an options object for granular control:
   * ```ts
   * smartPunctuation: { dashes: false } // quotes + ellipses only
   * ```
   */
  smartPunctuation?: boolean | SmartPunctuationOptions;
  /**
   * Parse raw HTML embedded in Markdown into real HAST element nodes.
   * Default: false.
   *
   * By default, inline and block HTML is kept as opaque `raw` nodes
   * (re-emitted verbatim on stringify). With `rawHtml: true`, the tree is
   * reparsed so raw HTML becomes structured `element`/`text`/`comment` nodes
   * with normalized properties, including tags that open in one raw block
   * and close in another. Positions are not preserved through the reparse,
   * except on the root, which keeps the document's own span.
   */
  rawHtml?: boolean;
}

export interface CompileOptions {
  /** MDAST plugins, in run order. A nested array runs its own plugins, in
   *  their own order, at the array's position. */
  mdastPlugins?: MdastPluginList;
  /** HAST plugins, in run order. Nests like `mdastPlugins`. */
  hastPlugins?: HastPluginList;
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
 * JS/JSX output options, accepted by both {@link mdxToJs} and
 * {@link markdownToJs}. Exported on its own so wrappers can expose the codegen
 * knobs without the shared pipeline fields.
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

export interface MarkdownToJsOptions extends CompileOptions, MdxOnlyOptions {}

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

/** Result of {@link markdownToJs}. */
export interface MarkdownToJsResult {
  /** Compiled JavaScript module source. */
  code: string;
  /** Frontmatter block at the start of the document, or `null` if none. */
  frontmatter: Frontmatter | null;
  /** Document-level data bag shared with plugins via `ctx.data`; the seeded `data` option if provided, else a fresh `{}`. */
  data: Data;
}

// Type helpers: detect whether any visitor in any plugin returns a Promise.
// Used to narrow the compile entry points to a sync return when every plugin
// is sync, while keeping the union when at least one visitor is async.

// "maybe" is for a declared type that admits both, where no call site can tell which it gets.
type CombineAsync<A> = "async" extends A ? "async" : "maybe" extends A ? "maybe" : "sync";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFn = (...args: any[]) => unknown;
type ReturnsPromise<F> = F extends AnyFn
  ? [Extract<ReturnType<F>, Promise<unknown>>] extends [never]
    ? "sync"
    : [Exclude<ReturnType<F>, Promise<unknown>>] extends [never]
      ? "async"
      : "maybe"
  : "sync";
type FieldIsAsync<V> = V extends AnyFn
  ? ReturnsPromise<V>
  : V extends { visit: infer F }
    ? ReturnsPromise<F>
    : V extends ReadonlyArray<infer Item>
      ? Item extends { visit: infer F }
        ? ReturnsPromise<F>
        : "sync"
      : "sync";
type AnyVisitorAsync<P> = {
  [K in keyof P]-?: FieldIsAsync<NonNullable<P[K]>>;
}[keyof P];
// `P extends unknown` forces distribution; `"async" extends AnyVisitorAsync<P>`
// alone would not, since the checked type is `"async"` rather than P. Undistributed,
// a union of plugins with differing visitor keys collapses to their common keys
// and the async visitor goes unseen.
type IsPluginAsync<P> = P extends unknown ? CombineAsync<AnyVisitorAsync<P>> : never;
// Depth-capped because `PluginEntry` is recursive: an uncapped walk never
// terminates on a value typed as the alias itself, which TS rejects outright as
// "type instantiation is excessively deep".
type ResolveInput<P, D extends readonly unknown[] = [0, 0, 0, 0, 0, 0, 0, 0]> = D extends readonly [
  unknown,
  ...infer Rest,
]
  ? P extends ReadonlyArray<infer Item>
    ? ResolveInput<Item, Rest>
    : // A factory taking a ctx is not assignable to `() => infer Def`, which would drop it as sync.
      P extends (...args: never[]) => infer Def
      ? ResolveInput<Def, Rest>
      : P
  : never;
type AnyInputAsync<Ps> =
  Ps extends ReadonlyArray<infer P> ? CombineAsync<IsPluginAsync<ResolveInput<P>>> : "sync";
// Indexed access, not `O extends { mdastPlugins: infer Ps }`: that is false for an optional property.
type OptionsAsync<O extends CompileOptions> = CombineAsync<
  AnyInputAsync<NonNullable<O["mdastPlugins"]>> | AnyInputAsync<NonNullable<O["hastPlugins"]>>
>;

type ResultFor<O extends CompileOptions, R> = {
  sync: R;
  async: Promise<R>;
  maybe: R | Promise<R>;
}[OptionsAsync<O>];

export function markdownToHtml(source: string): MarkdownToHtmlResult;
export function markdownToHtml<O extends CompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MarkdownToHtmlResult>;
export function markdownToHtml(
  source: string,
  options: CompileOptions = {},
): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> {
  const { features, fileURL, data = {} } = options;
  const mdastPlugins = normalizePlugins(
    options.mdastPlugins ?? [],
    "mdastPlugins",
    source,
    fileURL,
    "markdown",
    data,
  );
  const hastPlugins = normalizePlugins(
    options.hastPlugins ?? [],
    "hastPlugins",
    source,
    fileURL,
    "markdown",
    data,
  );
  const hastMayHaveStubs = hastPlugins.length > 0;
  const { parseOptions, convertOptions: nativeConvertOptions } = featuresToNative(features);

  // Fast path: no plugins → parse, convert, render, and frontmatter all happen
  // inside a single NAPI call. Skips 5 handle-NAPI roundtrips that the
  // plugin-capable path needs to keep the arena live across passes.
  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    const { html, frontmatter } = markdownToHtmlFast(source, parseOptions, nativeConvertOptions);
    return { html, frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null, data };
  }

  // Track source positions only when some plugin opts in with `position: true`;
  // otherwise the parse skips the LineIndex build and per-node line/column lookups.
  const trackPositions =
    mdastPlugins.some((p) => p.options?.position) || hastPlugins.some((p) => p.options?.position);

  // Fused tail for MDAST-plugins-only (no HAST plugins): after the MDAST
  // plugin pass returns its pending commands, apply + convert + render all
  // happen inside a single NAPI roundtrip. Saves the convert-to-hast handle
  // create + render + drop crossings the generic path makes separately.
  if (hastPlugins.length === 0) {
    const mdastHandle = createMdastHandle(source, parseOptions, trackPositions);
    try {
      const mdastResult = runMdastPluginsOnHandle(
        mdastHandle,
        mdastPlugins,
        fileURL,
        data,
        "markdown",
        true,
      );
      const finishMdast = (r: MdastPipelineResult): MarkdownToHtmlResult => {
        try {
          // Fused tail: apply pending commands (empty buffer is a Rust no-op),
          // extract frontmatter post-mutation, convert, render, all in one
          // NAPI roundtrip.
          const commands = r.pendingCommands ?? EMPTY_COMMAND_BUFFER;
          const { html, frontmatter, droppedTransforms } = applyMdastCommandsAndConvertAndRender(
            r.handle,
            commands,
            nativeConvertOptions,
          );
          if (droppedTransforms && r.lastPlugin)
            warnDroppedTransforms(r.lastPlugin, droppedTransforms, "mdast");
          return {
            html,
            frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null,
            data,
          };
        } finally {
          releaseHandle(r.handle, true);
        }
      };
      if (mdastResult instanceof Promise) {
        return mdastResult.then(finishMdast, (err) => {
          releaseHandle(mdastHandle, true);
          throw err;
        });
      }
      return finishMdast(mdastResult);
    } catch (err) {
      releaseHandle(mdastHandle, true);
      throw err;
    }
  }

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    false,
    fileURL,
    parseOptions,
    nativeConvertOptions,
    data,
    trackPositions,
  );

  const runHastThenRender = (
    r: HastWithFrontmatter,
  ): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> => {
    // Run all but the last HAST plugin normally (each one applies its
    // commands inline); collect the last plugin's commands and fuse apply +
    // render + handle-drop into one NAPI call.
    let collected: CollectedHastCommands | Promise<CollectedHastCommands>;
    try {
      collected = runHastPluginsCollectLast(
        r.hastHandle,
        hastPlugins,
        source,
        fileURL,
        data,
        "markdown",
      );
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (collected instanceof Promise) {
      return collected.then(
        (c) => finishHastRender(r.hastHandle, c, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return finishHastRender(r.hastHandle, collected, r.frontmatter);
  };

  const finishHastRender = (
    h: HastHandle,
    collected: CollectedHastCommands,
    frontmatter: Frontmatter | null,
  ): MarkdownToHtmlResult => {
    try {
      let html: string;
      if (collected.commands.length > 0) {
        const result = applyCommandsAndRenderHandle(h, collected.commands);
        if (result.droppedTransforms && collected.lastPlugin)
          warnDroppedTransforms(collected.lastPlugin, result.droppedTransforms, "hast");
        html = result.html;
      } else {
        html = renderHandle(h);
      }
      return { html, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
  };

  if (result instanceof Promise) return result.then(runHastThenRender);
  return runHastThenRender(result);
}

export function mdxToJs(source: string): MdxToJsResult;
export function mdxToJs<O extends MdxCompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MdxToJsResult>;
export function mdxToJs(
  source: string,
  options: MdxCompileOptions = {},
): MdxToJsResult | Promise<MdxToJsResult> {
  return toJsImpl(source, options, true);
}

/**
 * Compile plain Markdown to a JavaScript module: like {@link mdxToJs}, but
 * without MDX syntax: `{...}` expressions, JSX tags, and `import`/`export`
 * lines are ordinary Markdown. HTML has no JSX representation and is dropped;
 * enable `features: { rawHtml: true }` to parse it into real elements instead.
 */
export function markdownToJs(source: string): MarkdownToJsResult;
export function markdownToJs<O extends MarkdownToJsOptions>(
  source: string,
  options?: O,
): ResultFor<O, MarkdownToJsResult>;
export function markdownToJs(
  source: string,
  options: MarkdownToJsOptions = {},
): MarkdownToJsResult | Promise<MarkdownToJsResult> {
  return toJsImpl(source, options, false);
}

/** `mdx` picks the parser; the pipeline is identical from MDAST on. */
function toJsImpl(
  source: string,
  options: MdxCompileOptions,
  mdx: boolean,
): MdxToJsResult | Promise<MdxToJsResult> {
  const {
    mdastPlugins: mdastInput = [],
    hastPlugins: hastInput = [],
    features,
    fileURL,
    data = {},
    ...mdxFields
  } = options;
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";
  const mdastPlugins = normalizePlugins(
    mdastInput,
    "mdastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const hastPlugins = normalizePlugins(
    hastInput,
    "hastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const hastMayHaveStubs = hastPlugins.length > 0;
  const mdxOptions = mdxOptionsToNative(mdxFields);
  const { parseOptions, convertOptions: nativeConvertOptions } = featuresToNative(features);

  // Fast path: same trick as `markdownToHtml`. Parse → MDAST → HAST → JS plus
  // frontmatter extraction all happen inside a single NAPI call. Skips 5 of
  // the 6 handle-based crossings the plugin-capable path needs.
  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    const { code, frontmatter } = (mdx ? mdxToJsFast : markdownToJsFast)(
      source,
      parseOptions,
      mdxOptions,
      nativeConvertOptions,
    );
    return { code, frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null, data };
  }

  // Track positions only when a plugin opts in (see `markdownToHtml`).
  const trackPositions =
    mdastPlugins.some((p) => p.options?.position) || hastPlugins.some((p) => p.options?.position);

  // MDAST-plugins-only fused tail (no HAST plugins): apply + extract
  // frontmatter + convert + simplify + compile happen in one NAPI call.
  if (hastPlugins.length === 0) {
    const mdastHandle = mdx
      ? createMdxMdastHandle(source, parseOptions, trackPositions)
      : createMdastHandle(source, parseOptions, trackPositions);
    try {
      const mdastResult = runMdastPluginsOnHandle(
        mdastHandle,
        mdastPlugins,
        fileURL,
        data,
        mdx ? "mdx" : "markdown",
        true,
      );
      const finishMdast = (r: MdastPipelineResult): MdxToJsResult => {
        try {
          const commands = r.pendingCommands ?? EMPTY_COMMAND_BUFFER;
          const { code, frontmatter, droppedTransforms } = applyMdastCommandsAndConvertAndCompile(
            r.handle,
            commands,
            mdxOptions,
            nativeConvertOptions,
          );
          if (droppedTransforms && r.lastPlugin)
            warnDroppedTransforms(r.lastPlugin, droppedTransforms, "mdast");
          return {
            code,
            frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null,
            data,
          };
        } finally {
          releaseHandle(r.handle, true);
        }
      };
      if (mdastResult instanceof Promise) {
        return mdastResult.then(finishMdast, (err) => {
          releaseHandle(mdastHandle, true);
          throw err;
        });
      }
      return finishMdast(mdastResult);
    } catch (err) {
      releaseHandle(mdastHandle, true);
      throw err;
    }
  }

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    mdx,
    fileURL,
    parseOptions,
    nativeConvertOptions,
    data,
    trackPositions,
  );

  const runHastThenCompile = (r: HastWithFrontmatter): MdxToJsResult | Promise<MdxToJsResult> => {
    let collected: CollectedHastCommands | Promise<CollectedHastCommands>;
    try {
      collected = runHastPluginsCollectLast(
        r.hastHandle,
        hastPlugins,
        source,
        fileURL,
        data,
        mdx ? "mdx" : "markdown",
      );
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (collected instanceof Promise) {
      return collected.then(
        (c) => finishHastCompile(r.hastHandle, c, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return finishHastCompile(r.hastHandle, collected, r.frontmatter);
  };

  const finishHastCompile = (
    h: HastHandle,
    collected: CollectedHastCommands,
    frontmatter: Frontmatter | null,
  ): MdxToJsResult => {
    try {
      let code: string;
      if (collected.commands.length > 0) {
        const result = applyCommandsAndCompileHandle(h, collected.commands, mdxOptions);
        if (result.droppedTransforms && collected.lastPlugin)
          warnDroppedTransforms(collected.lastPlugin, result.droppedTransforms, "hast");
        code = result.code;
      } else {
        code = compileHandle(h, mdxOptions);
      }
      return { code, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
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
  mdastPlugins: MdastPluginDefinition[],
  mdx: boolean,
  fileURL: URL | undefined,
  parseOptions: number,
  nativeConvertOptions: NativeConvertOptions | undefined,
  data: Data,
  trackPositions: boolean,
): HastWithFrontmatter | Promise<HastWithFrontmatter> {
  if (mdastPlugins.length === 0) {
    const [hastHandle, raw] = mdx
      ? createMdxHastHandleWithFrontmatter(
          source,
          parseOptions,
          nativeConvertOptions,
          trackPositions,
        )
      : createHastHandleWithFrontmatter(source, parseOptions, nativeConvertOptions, trackPositions);
    return {
      hastHandle,
      frontmatter: raw ? { kind: raw.kind === "toml" ? "toml" : "yaml", value: raw.value } : null,
    };
  }

  const mdastHandle = mdx
    ? createMdxMdastHandle(source, parseOptions, trackPositions)
    : createMdastHandle(source, parseOptions, trackPositions);
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";

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
    const mdastResult = runMdastPluginsOnHandle(
      mdastHandle,
      mdastPlugins,
      fileURL,
      data,
      sourceFormat,
    );

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

/** Options for the step-by-step tree functions. */
export interface TreeOptions {
  features?: Features;
  /**
   * Record `position` on every node. Default: `true`.
   *
   * A unist position is three objects per node and roughly half a materialized
   * tree's memory, on top of the parser's line-index build. Pass `false` when
   * nothing downstream reads `node.position`.
   */
  position?: boolean;
}

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string, options: TreeOptions = {}): MdastNode {
  const handle = createMdastHandle(
    source,
    featuresToNative(options.features).parseOptions,
    options.position,
  );
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string, options: TreeOptions = {}): MdastNode {
  const handle = createMdxMdastHandle(
    source,
    featuresToNative(options.features).parseOptions,
    options.position,
  );
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string, options: TreeOptions = {}): HastNode {
  const { parseOptions, convertOptions } = featuresToNative(options.features);
  const handle = createHastHandle(source, parseOptions, convertOptions, options.position);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string, options: TreeOptions = {}): HastNode {
  const { parseOptions, convertOptions } = featuresToNative(options.features);
  const handle = createMdxHastHandle(source, parseOptions, convertOptions, options.position);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

export interface HtmlToHastOptions {
  /**
   * Parse as a fragment, so the returned `root` holds the string's own
   * top-level nodes instead of an implied `<html>`/`<head>`/`<body>`.
   *
   * @default false
   */
  fragment?: boolean;
  /**
   * The namespace a fragment's own top-level content parses in. `"svg"` reads
   * it as foreign content, so `<circle />` self-closes and lands in the SVG
   * namespace instead of being treated as an unknown HTML element.
   *
   * Ignored without `fragment: true`: a document always starts in HTML.
   *
   * @default "html"
   */
  space?: "html" | "svg";
}

/**
 * Parse an HTML string into a materialized hast tree: a `root` whose children
 * are the doctype (if any) and the implied `<html>` subtree, or the string's
 * own top-level nodes with `{ fragment: true }`. Only available in builds that
 * include the `from-html` feature.
 */
export function htmlToHast(html: string, options: HtmlToHastOptions = {}): HastNode {
  const handle = createHastHandleFromHtml(html, options.fragment, options.space);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}
