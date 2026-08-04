import type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
import type { HastVisitorInstance } from "./hast/hast-visitor.js";
import type { Data, SourceFormat } from "./types.js";

/**
 * What a plugin factory is told about the document, before it is parsed.
 *
 * Mirrors the fields a visitor's `ctx` exposes, minus everything that only
 * exists once there is a tree. Return a skip value from the factory to leave
 * the plugin out of the pipeline for this document.
 */
export interface PluginFactoryContext {
  /** The `fileURL` compile option, or `undefined` when none was given. */
  fileURL: URL | undefined;
  /** Which kind of document is being compiled. */
  sourceFormat: SourceFormat;
  /** The unparsed source. Intended for cheap checks, not for parsing Markdown. */
  source: string;
  /** The document-level data bag, before any plugin has run. */
  data: Data;
}

export type MdastPluginDefinition = MdastPluginInstance & { name: string };

export type HastPluginDefinition = HastVisitorInstance & { name: string };

/**
 * A definition reused across documents, or a factory called once per compile
 * so closures reset per document.
 */
export type MdastPluginInput = MdastPluginDefinition | (() => MdastPluginDefinition);

/**
 * A definition reused across documents, or a factory called once per compile
 * so closures reset per document.
 */
export type HastPluginInput = HastPluginDefinition | (() => HastPluginDefinition);

type PluginEntry<D> =
  | D
  | ((ctx: PluginFactoryContext) => PluginEntry<D>)
  | readonly PluginEntry<D>[]
  | null
  | undefined
  | false;

/** Entry accepted by `mdastPlugins`. */
export type MdastPluginEntry = PluginEntry<MdastPluginDefinition>;

/** Entry accepted by `hastPlugins`. */
export type HastPluginEntry = PluginEntry<HastPluginDefinition>;

/** Value accepted by the `mdastPlugins` option. */
export type MdastPluginList = readonly MdastPluginEntry[];

/** Value accepted by the `hastPlugins` option. */
export type HastPluginList = readonly HastPluginEntry[];

/** Bounds factory-in-factory nesting. Real presets nest one level; anything
 *  deeper is a factory that leads back to itself, which would otherwise recurse
 *  until the stack overflows. */
const MAX_FACTORY_DEPTH = 10;

/** The one place a plugin option becomes the definition array the pipeline
 *  runs. Factories resolve here and nowhere else, so each is called once per
 *  compile no matter how deeply it is nested. */
// The context is taken apart rather than passed whole so it can be built on
// the first factory encountered: a list with no factories allocates nothing.
export function normalizePlugins<D>(
  entries: readonly PluginEntry<D>[],
  option: string,
  source: string,
  fileURL: URL | undefined,
  sourceFormat: SourceFormat,
  data: Data,
): D[] {
  const out: D[] = [];
  let ctx: PluginFactoryContext | undefined;
  const walk = (entry: PluginEntry<D>, factoryDepth: number): void => {
    // Only these three, not every falsy value, so a stray `0` or `""` still
    // reaches the push below and surfaces as a bad plugin rather than vanishing.
    if (entry === null || entry === undefined || entry === false) return;
    if (Array.isArray(entry)) {
      for (const item of entry as readonly PluginEntry<D>[]) walk(item, factoryDepth);
      return;
    }
    if (typeof entry === "function") {
      if (factoryDepth === 0) {
        throw new Error(
          `${option}: plugin factory nesting is too deep — a factory most likely returns itself. ` +
            `A factory may return a plugin or a list of plugins, but that list must not lead back to the same factory.`,
        );
      }
      ctx ??= { fileURL, sourceFormat, source, data };
      walk((entry as (ctx: PluginFactoryContext) => PluginEntry<D>)(ctx), factoryDepth - 1);
      return;
    }
    out.push(entry as D);
  };
  for (const entry of entries) walk(entry, MAX_FACTORY_DEPTH);
  return out;
}

// Generic so the inferred plugin type preserves each visitor's *actual* return
// type. That lets the compile entry points distinguish sync plugins from async
// ones in their conditional return type.
export function defineMdastPlugin<P extends MdastPluginDefinition>(definition: P): P {
  if (!definition.name) {
    throw new Error("Plugin definition must have a name");
  }
  return definition;
}

export function defineHastPlugin<P extends HastPluginDefinition>(definition: P): P {
  if (!definition.name) {
    throw new Error("Plugin definition must have a name");
  }
  return definition;
}
