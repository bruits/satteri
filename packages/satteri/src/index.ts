// Public API: compile functions
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

// Plugin definitions
export { defineMdastPlugin, defineHastPlugin } from "./plugin.js";
export type {
  MdastPluginDefinition,
  HastPluginDefinition,
  MdastPluginInput,
  HastPluginInput,
} from "./plugin.js";

// Visitor types (for plugin authors)
export type {
  HastVisitorInstance,
  HastVisitorContext,
  HastFilteredVisitor,
  HastContent,
  EstreeProgram,
} from "./hast/hast-visitor.js";

// Node types
export type {
  MdastNode,
  HastNode,
  DataMap,
  Data,
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

// Visitor pipeline (for manual plugin execution)
export { visitMdastHandle, resolveMdastSubscriptions } from "./mdast/mdast-visitor.js";
export type {
  MdastPluginInstance,
  MdastVisitorContext,
  MdastContent,
} from "./mdast/mdast-visitor.js";
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
  serializeHandle,
  renderHandle,
  compileHandle,
  getHandleSource,
} from "#binding";

import {
  applyCommandsToMdastHandle as napiApplyCommandsToMdastHandle,
  applyCommandsAndConvertToHastHandle as napiApplyCommandsAndConvertToHastHandle,
  convertMdastToHastHandle as napiConvertMdastToHastHandle,
  dropHandle as napiDropHandle,
} from "#binding";
import type { AnyHandle } from "./handles.js";
import { markHandleMutated } from "./lazy-child-resolver.js";

// The raw NAPI mutators renumber or empty the arena; without the epoch bump a
// child stub retained past a manual-pipeline pass would silently snapshot the
// changed arena (or die with an opaque RangeError) instead of hitting the
// retention error.

export function applyCommandsToMdastHandle(handle: MdastHandle, commandBuf: Uint8Array): number {
  markHandleMutated(handle);
  return napiApplyCommandsToMdastHandle(handle, commandBuf);
}

export function convertMdastToHastHandle(
  handle: MdastHandle,
  convertOptions?: Parameters<typeof napiConvertMdastToHastHandle>[1],
): HastHandle {
  markHandleMutated(handle);
  return napiConvertMdastToHastHandle(handle, convertOptions);
}

export function dropHandle(handle: AnyHandle): void {
  markHandleMutated(handle);
  napiDropHandle(handle);
}

export function applyCommandsAndConvertToHastHandle(
  handle: MdastHandle,
  commandBuf: Uint8Array,
  convertOptions?: Parameters<typeof napiApplyCommandsAndConvertToHastHandle>[2],
): HastHandle {
  markHandleMutated(handle);
  return napiApplyCommandsAndConvertToHastHandle(handle, commandBuf, convertOptions);
}
