export { MdastReader, NodeType, NodeTypeName } from "./mdast-reader.js";
export { DataMap } from "./data-map.js";
export { materializeNode, materializeTree, TYPE_NAMES } from "./materializer.js";
export { visitMdast, MutationType } from "./visitor.js";
export { CommandBuffer, classifyReturn, resolveFieldId } from "./command-buffer.js";
export { definePlugin } from "./plugin.js";
export { createProcessor, ProcessorContext } from "./processor.js";
export {
  parseToBuffer,
  parseMdxToBuffer,
  parseToHastBuffer,
  parseMdxToHastBuffer,
  mdastBufferToHastBuffer,
  hastBufferToHtmlStr,
  compileMdx,
  compileMdxFromBuffer,
  compileMdxHighlighted,
  compileMdxFromBufferHighlighted,
  parseToHtml,
  parseMdxToHtml,
  applyMutations,
} from "../index.js";

// HAST support
export {
  HastReader,
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_TEXT,
  HAST_COMMENT,
  HAST_DOCTYPE,
  HAST_RAW,
  PROP_STRING,
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_COMMA_SEP,
} from "./hast-reader.js";
export type { HastProperty } from "./hast-reader.js";
export { materializeHastNode, materializeHastTree } from "./hast-materializer.js";
export type { HastNode } from "./hast-materializer.js";
export { visitHast } from "./hast-visitor.js";
export type {
  HastVisitorInstance,
  HastVisitorContext,
  VisitResult as HastVisitResult,
} from "./hast-visitor.js";

export type { MdastNode, Position, Point } from "./types.js";
