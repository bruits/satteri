// Shared MDAST types for the tryckeri JS layer.

/** A value that may be synchronous or a Promise. */
export type MaybePromise<T> = T | Promise<T>;

/**
 * Only awaits if the value is actually a thenable.
 * Avoids microtask overhead for sync plugin returns.
 */
export function maybeAwait<T>(value: MaybePromise<T>): MaybePromise<T> {
  if (value !== null && value !== undefined && typeof (value as { then?: unknown }).then === "function") {
    return value as Promise<T>;
  }
  return value as T;
}

export interface Point {
  offset: number;
  line: number;
  column: number;
}

export interface Position {
  start: Point;
  end: Point;
}

// Base for all materialized MDAST nodes.
// Type-specific properties are added lazily by the materializer.
export interface MdastNode {
  type: string;
  position: Position;
  /** Side-channel data (hProperties, id, etc.). Backed by DataMap. */
  data: Record<string, unknown> | null;
  /** Non-enumerable arena node ID. */
  _nodeId: number;
  // Optional on parent nodes
  children?: MdastNode[];
  // Optional type-specific fields (present depending on type)
  depth?: number;
  value?: string;
  lang?: string | null;
  meta?: string | null;
  url?: string;
  title?: string | null;
  alt?: string;
  ordered?: boolean;
  start?: number | null;
  spread?: boolean;
  checked?: boolean | null;
  identifier?: string;
  label?: string;
  referenceType?: string;
  align?: (string | null)[];
  name?: string | null;
  attributes?: MdxJsxAttributeUnion[];
}

export interface MdxJsxAttributeNode {
  type: "mdxJsxAttribute";
  name: string;
  value: string | MdxJsxAttributeValueExpressionNode | null;
}

export interface MdxJsxExpressionAttributeNode {
  type: "mdxJsxExpressionAttribute";
  value: string;
}

export interface MdxJsxAttributeValueExpressionNode {
  type: "mdxJsxAttributeValueExpression";
  value: string;
}

export type MdxJsxAttributeUnion = MdxJsxAttributeNode | MdxJsxExpressionAttributeNode;

export interface StringRefRaw {
  offset: number;
  len: number;
}

export interface ArenaNodeRaw {
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
