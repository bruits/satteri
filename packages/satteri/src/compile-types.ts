import type { HastPluginList, MdastPluginList } from "./plugin.js";
import type { Data } from "./types.js";

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
   * and close in another. Positions are not preserved through the reparse.
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

/** Options for the tree functions that stop at mdast. */
export interface MdastTreeOptions extends TreeOptions {
  /** MDAST plugins, in run order. A nested array runs its own plugins, in
   *  their own order, at the array's position. */
  mdastPlugins?: MdastPluginList;
  /** The document being processed, surfaced to plugins as `ctx.fileURL`. */
  fileURL?: URL;
  /** Initial document-level data bag, seeding `ctx.data` before any plugin
   *  runs. Used by reference and mutated in place. */
  data?: Data;
}

/** Options for the tree functions that run through to hast. */
export interface HastTreeOptions extends MdastTreeOptions {
  /** HAST plugins, in run order. Nests like `mdastPlugins`. */
  hastPlugins?: HastPluginList;
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

// Preserve synchronous return types when all configured plugins are synchronous.

// A union return type cannot reveal whether a particular call will return a promise.
type CombineAsync<A> = "async" extends A ? "async" : "maybe" extends A ? "maybe" : "sync";

type AnyFn = (...args: never[]) => unknown;
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
// Distribute over plugin unions so async visitors with distinct keys are not lost.
type IsPluginAsync<P> = P extends unknown ? CombineAsync<AnyVisitorAsync<P>> : never;
// Bound recursion so the recursive PluginEntry alias stays within TypeScript's instantiation limit.
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
type PluginBearingOptions = {
  mdastPlugins?: MdastPluginList;
  hastPlugins?: HastPluginList;
};

// Indexed access, not `O extends { mdastPlugins: infer Ps }`: that is false for an optional property.
type OptionsAsync<O extends PluginBearingOptions> = CombineAsync<
  AnyInputAsync<NonNullable<O["mdastPlugins"]>> | AnyInputAsync<NonNullable<O["hastPlugins"]>>
>;

export type ResultFor<O extends PluginBearingOptions, R> = {
  sync: R;
  async: Promise<R>;
  maybe: R | Promise<R>;
}[OptionsAsync<O>];
