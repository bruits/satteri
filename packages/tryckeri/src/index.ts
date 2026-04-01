// Public API — compile functions
export { compileMarkdownToHtml, compileMdxToJs } from "./compile.js";
export type { CompileOptions, OptimizeStaticConfig } from "./compile.js";

// Plugin definitions
export { defineMdastPlugin, defineHastPlugin } from "./plugin.js";
export type { MdastPluginDefinition, HastPluginDefinition } from "./plugin.js";

// Visitor types (for plugin authors)
export type {
  HastVisitorInstance,
  HastVisitorContext,
  HastVisitResult,
  HastFilteredVisitor,
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
