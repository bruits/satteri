import type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
import type { HastVisitorInstance } from "./hast/hast-visitor.js";
import type { Data, SourceFormat } from "./types.js";

/**
 * What a plugin factory is told about the document, before it is parsed.
 *
 * Return `null`, `undefined` or `false` from the factory to leave the plugin
 * out of the pipeline for this document.
 */
export interface PluginFactoryContext {
  /** The `fileURL` compile option, or `undefined` when none was given. */
  readonly fileURL: URL | undefined;
  /** Which kind of document is being compiled. */
  readonly sourceFormat: SourceFormat;
  /** The unparsed source. Intended for cheap checks, not for parsing Markdown. */
  readonly source: string;
  /** The document-level data bag, before any plugin has run. */
  readonly data: Data;
}

export type MdastPluginDefinition = MdastPluginInstance & { name: string };

export type HastPluginDefinition = HastVisitorInstance & { name: string };

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

/** Older name for {@link MdastPluginEntry}. */
export type MdastPluginInput = MdastPluginEntry;

/** Older name for {@link HastPluginEntry}. */
export type HastPluginInput = HastPluginEntry;

/** Bounds factory-in-factory nesting. Real presets nest one level; anything
 *  deeper is a factory that leads back to itself, which would otherwise recurse
 *  until the stack overflows. */
const MAX_FACTORY_DEPTH = 10;

/** The one place a plugin option becomes the definition array the pipeline
 *  runs. Factories resolve here and nowhere else, so each is called once per
 *  compile no matter how deeply it is nested. */
export function normalizePlugins<D>(
  entries: readonly PluginEntry<D>[],
  option: string,
  source: string,
  fileURL: URL | undefined,
  sourceFormat: SourceFormat,
  data: Data,
): D[] {
  const out: D[] = [];
  // Built lazily so a list with no factories allocates no context.
  let ctx: PluginFactoryContext | undefined;
  const walk = (entry: PluginEntry<D>, factoryDepth: number): void => {
    if (entry === null || entry === undefined || entry === false) return;
    if (Array.isArray(entry)) {
      for (const item of entry as readonly PluginEntry<D>[]) walk(item, factoryDepth);
      return;
    }
    if (typeof entry === "function") {
      if (factoryDepth === 0) {
        throw new Error(
          `${option}: plugin factory nesting is too deep. A factory most likely returns itself. ` +
            `A factory may return a plugin or a list of plugins, but that list must not lead back to the same factory.`,
        );
      }
      // `data` stays mutable on purpose: it is the live bag the visitors share.
      ctx ??= Object.freeze({ fileURL, sourceFormat, source, data });
      walk((entry as (ctx: PluginFactoryContext) => PluginEntry<D>)(ctx), factoryDepth - 1);
      return;
    }
    if (typeof entry !== "object") {
      throw new Error(`${option}: expected a plugin, a factory, a list, or null/undefined/false`);
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
