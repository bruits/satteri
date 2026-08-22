// Public API: compile functions
export {
  markdownToHtml,
  markdownToJs,
  mdxToJs,
  mdxToJsMany,
  evaluate,
  markdownToMdast,
  mdxToMdast,
  markdownToHast,
  mdxToHast,
  htmlToHast,
} from "./compile.js";
export type {
  CompileOptions,
  MdxCompileOptions,
  MdxOnlyOptions,
  MarkdownToJsOptions,
  EvaluateOptions,
  OptimizeStaticConfig,
  Features,
  TreeOptions,
  HtmlToHastOptions,
  SmartPunctuationOptions,
  Frontmatter,
  MarkdownToHtmlResult,
  MarkdownToJsResult,
  MdxToJsResult,
  MdxToJsManyResult,
  MdxToJsManyOptions,
} from "./compile.js";

// Plugin definitions
export { defineMdastPlugin, defineHastPlugin } from "./plugin.js";
export type {
  MdastPluginDefinition,
  HastPluginDefinition,
  MdastPluginInput,
  HastPluginInput,
  MdastPluginEntry,
  HastPluginEntry,
  MdastPluginList,
  HastPluginList,
  PluginFactoryContext,
} from "./plugin.js";

// Visitor types (for plugin authors)
export type {
  HastVisitorInstance,
  HastVisitorContext,
  HastFilteredVisitor,
  HastContent,
  HastParentContent,
  RawHastContent,
  RawHtmlHastContent,
  EstreeProgram,
} from "./hast/hast-visitor.js";

// Node types
export type {
  MdastNode,
  HastNode,
  Custom,
  DataMap,
  Data,
  SourceFormat,
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
export { normalizePlugins } from "./plugin.js";
export {
  visitMdastHandle,
  visitMdastHook,
  resolveMdastSubscriptions,
} from "./mdast/mdast-visitor.js";
export type {
  MdastPluginInstance,
  MdastVisitorContext,
  MdastContent,
  MdastTarget,
  MdastParentContent,
  MdastDiagnostic,
  MdastHookFn,
  RawMdastContent,
  RawHtmlMdastContent,
} from "./mdast/mdast-visitor.js";
export {
  visitHastHandle,
  visitHastHook,
  resolveSubscriptions as resolveHastSubscriptions,
} from "./hast/hast-visitor.js";
export type { HastDiagnostic, HastHookFn } from "./hast/hast-visitor.js";

// Step-by-step API: readers, materializers, and handle functions
export { MdastReader } from "./mdast/mdast-reader.js";
export { materializeMdastTree } from "./mdast/mdast-materializer.js";
export { HastReader } from "./hast/hast-reader.js";
export { materializeHastTree } from "./hast/hast-materializer.js";

export { serializeHandle, renderHandle, compileHandle, getHandleSource } from "#binding";

import {
  applyCommandsToMdastHandle as napiApplyCommandsToMdastHandle,
  applyCommandsAndConvertToHastHandle as napiApplyCommandsAndConvertToHastHandle,
  convertMdastToHastHandle as napiConvertMdastToHastHandle,
  createHastHandle as napiCreateHastHandle,
  createMdastHandle as napiCreateMdastHandle,
  createMdxHastHandle as napiCreateMdxHastHandle,
  createMdxMdastHandle as napiCreateMdxMdastHandle,
  dropHandle as napiDropHandle,
} from "#binding";
import { featuresToNative } from "./compile.js";
import type { Features } from "./compile.js";
import type { AnyHandle } from "./handles.js";
import { markHandleMutated } from "./lazy-child-resolver.js";

type NativeConvertOptions = NonNullable<Parameters<typeof napiCreateHastHandle>[2]>;

// The napi creators take pre-packed parser bits; these keep `Features` the public shape.

export function createMdastHandle(
  source: string,
  features?: Features,
  trackPositions?: boolean,
): MdastHandle {
  return napiCreateMdastHandle(source, featuresToNative(features).parseOptions, trackPositions);
}

export function createMdxMdastHandle(
  source: string,
  features?: Features,
  trackPositions?: boolean,
): MdastHandle {
  return napiCreateMdxMdastHandle(source, featuresToNative(features).parseOptions, trackPositions);
}

export function createHastHandle(
  source: string,
  features?: Features,
  convertOptions?: NativeConvertOptions,
  trackPositions?: boolean,
): HastHandle {
  const native = featuresToNative(features);
  return napiCreateHastHandle(
    source,
    native.parseOptions,
    mergeConvertOptions(native.convertOptions, convertOptions),
    trackPositions,
  );
}

export function createMdxHastHandle(
  source: string,
  features?: Features,
  convertOptions?: NativeConvertOptions,
  trackPositions?: boolean,
): HastHandle {
  const native = featuresToNative(features);
  return napiCreateMdxHastHandle(
    source,
    native.parseOptions,
    mergeConvertOptions(native.convertOptions, convertOptions),
    trackPositions,
  );
}

function mergeConvertOptions(
  fromFeatures: NativeConvertOptions | undefined,
  explicit: NativeConvertOptions | undefined,
): NativeConvertOptions | undefined {
  if (fromFeatures === undefined) return explicit;
  if (explicit === undefined) return fromFeatures;
  return { ...fromFeatures, ...explicit };
}

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
