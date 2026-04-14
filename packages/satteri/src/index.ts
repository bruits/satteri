// Public API: compile functions
export {
  markdownToHtml,
  mdxToJs,
  markdownToMdast,
  mdxToMdast,
  markdownToHast,
  mdxToHast,
} from "./compile.js";
export type {
  CompileOptions,
  MdxCompileOptions,
  OptimizeStaticConfig,
  Features,
} from "./compile.js";

// Plugin definitions
export { defineMdastPlugin, defineHastPlugin } from "./plugin.js";
export type { MdastPluginDefinition, HastPluginDefinition } from "./plugin.js";

// Visitor types (for plugin authors)
export type {
  HastVisitorInstance,
  HastVisitorContext,
  HastFilteredVisitor,
  EstreeProgram,
} from "./hast/hast-visitor.js";

// Node types
export type {
  MdastNode,
  HastNode,
  Position,
  Point,
  MdxJsxAttributeNode,
  MdxJsxExpressionAttributeNode,
  MdxJsxAttributeValueExpressionNode,
  MdxJsxAttributeUnion,
} from "./types.js";

// Visitor pipeline (for manual plugin execution)
export { visitMdastHandle, resolveMdastSubscriptions } from "./mdast/mdast-visitor.js";
export type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
export {
  visitHastHandle,
  resolveSubscriptions as resolveHastSubscriptions,
} from "./hast/hast-visitor.js";

// Step-by-step API: readers, materializers, and handle functions
export { MdastReader } from "./mdast/mdast-reader.js";
export { materializeMdastTree } from "./mdast/mdast-materializer.js";
export { HastReader } from "./hast/hast-reader.js";
export { materializeHastTree } from "./hast/hast-materializer.js";

export {
  createMdastHandle,
  createMdxMdastHandle,
  createHastHandle,
  createMdxHastHandle,
  convertMdastToHastHandle,
  serializeMdastHandle,
  serializeHandle,
  renderHandle,
  compileHandle,
  dropHandle,
  applyCommandsToMdastHandle,
  applyCommandsAndConvertToHastHandle,
  getHandleSource,
} from "#binding";
