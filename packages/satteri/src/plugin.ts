import type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
import type { HastVisitorInstance } from "./hast/hast-visitor.js";

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

type PluginEntry<D> = D | (() => D) | readonly PluginEntry<D>[];

/** Entry accepted by `mdastPlugins`. */
export type MdastPluginEntry = PluginEntry<MdastPluginDefinition>;

/** Entry accepted by `hastPlugins`. */
export type HastPluginEntry = PluginEntry<HastPluginDefinition>;

/** Value accepted by the `mdastPlugins` option. */
export type MdastPluginList = readonly MdastPluginEntry[];

/** Value accepted by the `hastPlugins` option. */
export type HastPluginList = readonly HastPluginEntry[];

/** The one place a plugin option becomes the definition array the pipeline
 *  runs. Factories resolve here and nowhere else, so each is called once per
 *  compile no matter how deeply it is nested. */
export function normalizePlugins<D>(entries: readonly PluginEntry<D>[]): D[] {
  const out: D[] = [];
  const walk = (list: readonly PluginEntry<D>[]): void => {
    for (const entry of list) {
      if (Array.isArray(entry)) walk(entry as readonly PluginEntry<D>[]);
      else out.push(typeof entry === "function" ? (entry as () => D)() : (entry as D));
    }
  };
  walk(entries);
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
