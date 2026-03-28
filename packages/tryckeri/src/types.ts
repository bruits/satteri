// Shared types for the tryckeri JS layer.
//
// Uses standard @types/mdast, @types/hast, and mdast-util-mdx-* packages
// for AST node types. Extension types (toml, math, raw) are augmented here.

import type { Position } from "unist";
import type { Literal as MdastLiteral, Nodes as MdastStdNodes } from "mdast";
import type { Nodes as HastStdNodes } from "hast";

// Re-export standard position types from unist.
export type { Position, Point } from "unist";

// Re-export MDX types from their canonical packages.
// Importing these also registers them in the mdast/hast content maps
// via module augmentation (declare module 'mdast' / 'hast').
export type {
  MdxJsxFlowElement,
  MdxJsxTextElement,
  MdxJsxAttribute as MdxJsxAttributeNode,
  MdxJsxExpressionAttribute as MdxJsxExpressionAttributeNode,
  MdxJsxAttributeValueExpression as MdxJsxAttributeValueExpressionNode,
} from "mdast-util-mdx-jsx";
export type { MdxFlowExpression, MdxTextExpression } from "mdast-util-mdx-expression";
export type { MdxjsEsm } from "mdast-util-mdxjs-esm";

import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "mdast-util-mdx-jsx";

export type MdxJsxAttributeUnion = MdxJsxAttribute | MdxJsxExpressionAttribute;

// ── Extension mdast node types (not covered by standard packages) ───────────

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

declare module "mdast" {
  interface FrontmatterContentMap {
    toml: Toml;
  }
  interface RootContentMap {
    toml: Toml;
    math: MathNode;
    inlineMath: InlineMath;
  }
  interface PhrasingContentMap {
    inlineMath: InlineMath;
  }
  interface BlockContentMap {
    math: MathNode;
  }
}

// ── Extension hast node types ───────────────────────────────────────────────
// The standard mdx packages augment hast with mdxJsxFlowElement/
// mdxJsxTextElement and mdxFlowExpression/mdxTextExpression. We only need
// to register "raw" here since it has no standard package.

export interface HastRaw {
  type: "raw";
  value: string;
}

declare module "hast" {
  interface RootContentMap {
    raw: HastRaw;
  }
  interface ElementContentMap {
    raw: HastRaw;
  }
}

// ── Materialized node types (with internal _nodeId) ─────────────────────────

/** @internal Non-enumerable arena node ID added to all materialized nodes. */
interface NodeId {
  /** @internal */
  _nodeId: number;
}

/**
 * Materialized mdast node — a standard `mdast.Nodes` discriminated union with
 * an internal arena tracking ID. Narrow by `node.type` to access type-specific
 * properties (e.g. `depth` on `"heading"`, `url` on `"link"`).
 */
export type MdastNode = MdastStdNodes & NodeId;

/**
 * Materialized hast node — a standard `hast.Nodes` discriminated union with
 * an internal arena tracking ID. Narrow by `node.type` to access type-specific
 * properties (e.g. `tagName` on `"element"`, `value` on `"text"`).
 */
export type HastNode = HastStdNodes & NodeId;

// ── Internal binary format types ────────────────────────────────────────────

export interface StringRefRaw {
  offset: number;
  len: number;
}

export interface MdastNodeRaw {
  id: number;
  type: number;
  typeName: string;
  parent: number;
  position: Position;
  childrenStart: number;
  childrenCount: number;
  dataOffset: number;
  dataLen: number;
}

export interface BufferHeader {
  version: number;
  nodeStructSize: number;
  nodeCount: number;
  nodesOffset: number;
  childrenCount: number;
  childrenOffset: number;
  typeDataLen: number;
  typeDataOffset: number;
  sourceLen: number;
  sourceOffset: number;
}
