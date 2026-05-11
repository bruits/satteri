import type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
import type { HastVisitorInstance } from "./hast/hast-visitor.js";

export type MdastPluginDefinition = MdastPluginInstance & { name: string };

export type HastPluginDefinition = HastVisitorInstance & { name: string };

/**
 * Entry accepted by `mdastPlugins`: a definition reused across documents,
 * or a factory called once per compile so closures reset per document.
 */
export type MdastPluginInput = MdastPluginDefinition | (() => MdastPluginDefinition);

/**
 * Entry accepted by `hastPlugins`: a definition reused across documents,
 * or a factory called once per compile so closures reset per document.
 */
export type HastPluginInput = HastPluginDefinition | (() => HastPluginDefinition);

// Generic so the inferred plugin type preserves each visitor's *actual* return
// type. That lets call sites of `markdownToHtml`/`mdxToJs` distinguish sync
// plugins from async ones in their conditional return type.
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
