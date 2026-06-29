import type { Position } from "unist";
import type {
  Literal as MdastLiteral,
  Nodes as MdastStdNodes,
  Parent as MdastParent,
  PhrasingContent,
} from "mdast";
import type { Literal as HastLiteral, Nodes as HastStdNodes } from "hast";

// Re-export standard position types from unist.
export type { Position, Point } from "unist";

// Re-export MDX types from our local definitions.
// Importing these also registers them in the mdast/hast content maps
// via module augmentation (declare module 'mdast' / 'hast').
export type {
  MdxJsxFlowElement,
  MdxJsxTextElement,
  MdxJsxAttribute as MdxJsxAttributeNode,
  MdxJsxExpressionAttribute as MdxJsxExpressionAttributeNode,
  MdxJsxAttributeValueExpression as MdxJsxAttributeValueExpressionNode,
  MdxFlowExpression,
  MdxTextExpression,
  MdxjsEsm,
  MdxJsxFlowElementHast,
  MdxJsxTextElementHast,
  MdxFlowExpressionHast,
  MdxTextExpressionHast,
  MdxjsEsmHast,
} from "./mdx-types.js";

// Re-export directive types (also augments the mdast content maps).
export type {
  ContainerDirective,
  LeafDirective,
  TextDirective,
  DirectiveAttributes,
} from "./directive-types.js";

import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "./mdx-types.js";

export type MdxJsxAttributeUnion = MdxJsxAttribute | MdxJsxExpressionAttribute;

export interface Toml extends MdastLiteral {
  type: "toml";
}

export interface MathNode extends MdastLiteral {
  type: "math";
  meta?: string | null | undefined;
}

export interface InlineMath extends MdastLiteral {
  type: "inlineMath";
}

export interface Superscript extends MdastParent {
  type: "superscript";
  children: PhrasingContent[];
}

export interface Subscript extends MdastParent {
  type: "subscript";
  children: PhrasingContent[];
}

declare module "mdast" {
  interface FrontmatterContentMap {
    toml: Toml;
  }
  interface RootContentMap {
    toml: Toml;
    math: MathNode;
    inlineMath: InlineMath;
    superscript: Superscript;
    subscript: Subscript;
  }
  interface PhrasingContentMap {
    inlineMath: InlineMath;
    superscript: Superscript;
    subscript: Subscript;
  }
  interface BlockContentMap {
    math: MathNode;
  }
}

// The standard mdx packages augment hast with mdxJsxFlowElement/
// mdxJsxTextElement and mdxFlowExpression/mdxTextExpression. We only need
// to register "raw" here since it has no standard package.

export interface HastRaw extends HastLiteral {
  type: "raw";
}

declare module "hast" {
  interface RootContentMap {
    raw: HastRaw;
  }
  interface ElementContentMap {
    raw: HastRaw;
  }
}

/**
 * Materialized mdast node, a standard `mdast.Nodes` discriminated union.
 * Narrow by `node.type` to access type-specific properties
 * (e.g. `depth` on `"heading"`, `url` on `"link"`).
 */
export type MdastNode = MdastStdNodes;

/**
 * Materialized hast node, a standard `hast.Nodes` discriminated union.
 * Narrow by `node.type` to access type-specific properties
 * (e.g. `tagName` on `"element"`, `value` on `"text"`).
 */
export type HastNode = HastStdNodes;

/**
 * Registry for typing well-known keys on the plugin data bag. Augment it to
 * give specific `ctx.data` / `result.data` keys a type:
 *
 * ```ts
 * declare module "satteri" {
 *   interface DataMap {
 *     toc: TocEntry[];
 *   }
 * }
 * ```
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface DataMap {}

/**
 * The document-level plugin data bag (`ctx.data` and `result.data`): any
 * string key holding any value, plus the typed keys registered in {@link DataMap}.
 */
export type Data = Record<string, unknown> & Partial<DataMap>;

/** @internal Node with arena tracking ID, only used inside the library. */
export type MdastNodeInternal = MdastStdNodes & { _nodeId: number };
/** @internal */
export type HastNodeInternal = HastStdNodes & { _nodeId: number };

export interface StringRefRaw {
  offset: number;
  len: number;
}

export interface MdastNodeRaw {
  id: number;
  type: number;
  typeName: string;
  parent: number;
  position: Position | undefined;
  childrenStart: number;
  childrenCount: number;
  dataOffset: number;
  dataLen: number;
}

export interface BufferHeader {
  nodeStructSize: number;
  nodeCount: number;
  nodesOffset: number;
  childrenCount: number;
  childrenOffset: number;
  typeDataLen: number;
  typeDataOffset: number;
  stringPoolLen: number;
  stringPoolOffset: number;
  /** Number of nodes that carry an extra JSON `data` blob. */
  nodeDataCount: number;
  /** Offset of the node-data section: `[id u32][len u32][bytes...]` repeated. */
  nodeDataOffset: number;
}
