export {
  markdownToHtml,
  mdxToJs,
  evaluate,
  markdownToMdast,
  mdxToMdast,
  markdownToHast,
  mdxToHast,
} from "./compile.js";
export type {
  CompileOptions,
  MdxCompileOptions,
  MdxOnlyOptions,
  EvaluateOptions,
  OptimizeStaticConfig,
  Features,
  SmartPunctuationOptions,
  Frontmatter,
  MarkdownToHtmlResult,
  MdxToJsResult,
} from "./compile.js";

export { defineMdastPlugin, defineHastPlugin } from "./plugin.js";
export type {
  MdastPluginDefinition,
  HastPluginDefinition,
  MdastPluginInput,
  HastPluginInput,
} from "./plugin.js";

export type {
  HastVisitorInstance,
  HastVisitorContext,
  HastFilteredVisitor,
  EstreeProgram,
} from "./hast/hast-visitor.js";

export type {
  MdastNode,
  HastNode,
  Position,
  Point,
  MdxJsxAttributeNode,
  MdxJsxExpressionAttributeNode,
  MdxJsxAttributeValueExpressionNode,
  MdxJsxAttributeUnion,
  // MDX mdast node types (mdast plugin visitors hand these)
  MdxJsxFlowElement,
  MdxJsxTextElement,
  MdxFlowExpression,
  MdxTextExpression,
  MdxjsEsm,
  // MDX hast node types (hast plugin visitors hand these)
  MdxJsxFlowElementHast,
  MdxJsxTextElementHast,
  MdxFlowExpressionHast,
  MdxTextExpressionHast,
  MdxjsEsmHast,
} from "./types.js";

export { visitMdastHandle, resolveMdastSubscriptions } from "./mdast/mdast-visitor.js";
export type { MdastPluginInstance } from "./mdast/mdast-visitor.js";
export {
  visitHastHandle,
  resolveSubscriptions as resolveHastSubscriptions,
} from "./hast/hast-visitor.js";

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
  serializeHandle,
  renderHandle,
  compileHandle,
  dropHandle,
  applyCommandsToMdastHandle,
  applyCommandsAndConvertToHastHandle,
  getHandleSource,
} from "#binding";
