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
  /** The unparsed source, minus a leading BOM as the parser sees it. Intended
   *  for cheap checks, not for parsing Markdown. */
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

/** Alias for {@link MdastPluginEntry}. */
export type MdastPluginInput = MdastPluginEntry;

/** Alias for {@link HastPluginEntry}. */
export type HastPluginInput = HastPluginEntry;

// Bound recursive factories so a self-referential preset cannot overflow the stack.
const MAX_FACTORY_DEPTH = 10;

/** Resolve nested entries and invoke each factory once per compile. */
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
      ctx ??= Object.freeze({
        fileURL,
        sourceFormat,
        // The parser drops a leading BOM, so `ctx.source` in a visitor lacks it too.
        source: source.startsWith("\uFEFF") ? source.slice(1) : source,
        data,
      });
      walk((entry as (ctx: PluginFactoryContext) => PluginEntry<D>)(ctx), factoryDepth - 1);
      return;
    }
    if (typeof entry !== "object") {
      throw new Error(`${option}: expected a plugin, a factory, a list, or null/undefined/false`);
    }
    if (typeof (entry as { then?: unknown }).then === "function") {
      throw new Error(
        `${option}: a Promise is not a plugin. Plugin factories must be synchronous; ` +
          `await the value first and pass the plugin itself.`,
      );
    }
    out.push(entry as D);
  };
  for (const entry of entries) walk(entry, MAX_FACTORY_DEPTH);
  return out;
}

// Preserve each visitor’s inferred return type so compile results can distinguish sync from async plugins.
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
