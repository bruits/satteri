import type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
import type { HastVisitorInstance } from "./hast/hast-visitor.js";

export interface MdastPluginDefinition {
  name: string;
  createOnce(): MdastPluginInstance;
}

export interface HastPluginDefinition {
  name: string;
  createOnce(): HastVisitorInstance;
}

export function defineMdastPlugin(definition: MdastPluginDefinition): MdastPluginDefinition {
  if (!definition.name) {
    throw new Error("Plugin definition must have a name");
  }
  if (typeof definition.createOnce !== "function") {
    throw new Error("Plugin definition must have a createOnce function");
  }
  return definition;
}

export function defineHastPlugin(definition: HastPluginDefinition): HastPluginDefinition {
  if (!definition.name) {
    throw new Error("Plugin definition must have a name");
  }
  if (typeof definition.createOnce !== "function") {
    throw new Error("Plugin definition must have a createOnce function");
  }
  return definition;
}
