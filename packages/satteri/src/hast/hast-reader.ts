import type { Position } from "unist";
import { ArenaReader } from "../arena-reader.js";
import {
  FIELD,
  KIND_HAST,
  W_CHILDREN_COUNT,
  W_CHILDREN_START,
  W_DATA_LEN,
  W_DATA_OFFSET,
  W_PARENT,
  W_START_OFFSET,
} from "../generated/arena-layout.js";
import { decodeMdxJsxAttr } from "../mdx-attr.js";
import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "../mdx-types.js";
import type { ArenaWire, BufferHeader } from "../types.js";
import { NAME_TO_TYPE } from "./generated/node-types.js";

export type { MdxJsxAttribute, MdxJsxExpressionAttribute };

export const HAST_ROOT = NAME_TO_TYPE.root!;
export const HAST_ELEMENT = NAME_TO_TYPE.element!;
export const HAST_TEXT = NAME_TO_TYPE.text!;
export const HAST_COMMENT = NAME_TO_TYPE.comment!;
export const HAST_RAW = NAME_TO_TYPE.raw!;
export const HAST_MDX_JSX_ELEMENT = NAME_TO_TYPE.mdxJsxFlowElement!;
export const HAST_MDX_JSX_TEXT_ELEMENT = NAME_TO_TYPE.mdxJsxTextElement!;
export const HAST_MDX_FLOW_EXPRESSION = NAME_TO_TYPE.mdxFlowExpression!;
export const HAST_MDX_ESM = NAME_TO_TYPE.mdxjsEsm!;
export const HAST_MDX_TEXT_EXPRESSION = NAME_TO_TYPE.mdxTextExpression!;

export interface HastProperty {
  name: string;
  value: string | number | boolean | (string | number)[];
}

export class HastReader {
  readonly #arena: ArenaReader;
  // Typed arrays avoid DataView getter overhead in unoptimized V8 tiers.
  readonly #u8: Uint8Array;
  readonly #u32: Uint32Array;
  readonly #nodesB: number;
  readonly #nodesW: number;
  readonly #strideB: number;
  readonly #strideW: number;
  readonly #childrenW: number;
  readonly #typeDataB: number;

  constructor(buffer: ArrayBuffer | Uint8Array) {
    const arena = new ArenaReader(buffer, KIND_HAST);
    this.#arena = arena;
    this.#u8 = arena.u8;
    this.#u32 = arena.u32;
    const header = arena.header;
    this.#nodesB = header.nodesOffset;
    this.#nodesW = header.nodesOffset >> 2;
    this.#strideB = header.nodeStructSize;
    this.#strideW = header.nodeStructSize >> 2;
    this.#childrenW = header.childrenOffset >> 2;
    this.#typeDataB = header.typeDataOffset;
  }

  /** @internal Memoized wire view for the fused decoder. */
  getWire(): ArenaWire {
    return this.#arena.getWire();
  }

  /** Per-node JSON data, or null when absent. */
  getNodeData(nodeId: number): string | null {
    return this.#arena.getNodeData(nodeId);
  }

  /** @internal null when no node carries data. */
  getNodeDataTable(): ReadonlyMap<number, string> | null {
    return this.#arena.getNodeDataTable();
  }

  get nodeCount(): number {
    return this.#arena.header.nodeCount;
  }

  get header(): BufferHeader {
    return { ...this.#arena.header };
  }

  /** Full string pool, including interned strings; use ctx.source for the original document. */
  getStringPool(): string {
    return this.#arena.getStringPool();
  }

  /** Wire string refs use UTF-16 units, so substring preserves their offsets. */
  getString(offset: number, len: number): string {
    return this.#arena.getString(offset, len);
  }

  /** A zero start line marks a synthesized node with no source range (unist
   *  lines are 1-based), surfaced as `undefined`. */
  getPosition(nodeId: number): Position | undefined {
    const u32 = this.#u32;
    const b = this.#nodesW + nodeId * this.#strideW + W_START_OFFSET;
    const startLine = u32[b + 2] ?? 0;
    if (startLine === 0) return undefined;
    return {
      start: { offset: u32[b] ?? 0, line: startLine, column: u32[b + 3] ?? 0 },
      end: { offset: u32[b + 1] ?? 0, line: u32[b + 4] ?? 0, column: u32[b + 5] ?? 0 },
    };
  }

  /** Get the node_type byte for a given node ID. */
  getNodeType(nodeId: number): number {
    return this.#u8[this.#nodesB + nodeId * this.#strideB + FIELD.node_type] ?? 0;
  }

  /** Get the parent id for a given node (0xffffffff at the root). */
  getParentId(nodeId: number): number {
    return this.#u32[this.#nodesW + nodeId * this.#strideW + W_PARENT] ?? 0;
  }

  getChildrenCount(nodeId: number): number {
    return this.#u32[this.#nodesW + nodeId * this.#strideW + W_CHILDREN_COUNT] ?? 0;
  }

  getChildIds(nodeId: number): number[] {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    const childrenStart = u32[w + W_CHILDREN_START] ?? 0;
    const childrenCount = u32[w + W_CHILDREN_COUNT] ?? 0;
    if (childrenCount === 0) return [];
    const base = this.#childrenW + childrenStart;
    const ids: number[] = [];
    for (let i = 0; i < childrenCount; i++) {
      ids.push(u32[base + i] ?? 0);
    }
    return ids;
  }

  /** Get the raw type_data bytes for a node. */
  getTypeData(nodeId: number): Uint8Array {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    const dataOffset = u32[w + W_DATA_OFFSET] ?? 0;
    const dataLen = u32[w + W_DATA_LEN] ?? 0;
    if (dataLen === 0) return new Uint8Array(0);
    const start = this.#typeDataB + dataOffset;
    return this.#u8.subarray(start, start + dataLen);
  }

  /** Byte position of a node's `type_data`, relative to this reader's view, or
   *  `-1` when the node stores fewer than `min` bytes. */
  #typeDataAt(nodeId: number, min: number): number {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    if ((u32[w + W_DATA_LEN] ?? 0) < min) return -1;
    return this.#typeDataB + (u32[w + W_DATA_OFFSET] ?? 0);
  }

  /** A zero-length element name represents an MDX fragment. */
  getMdxJsxElementData(nodeId: number): {
    name: string | null;
    attributes: (MdxJsxAttribute | MdxJsxExpressionAttribute)[];
  } {
    const at = this.#typeDataAt(nodeId, 16);
    if (at === -1) {
      return { name: null, attributes: [] };
    }
    const u32 = this.#u32;
    const w = at >> 2;
    const nameLen = u32[w + 1] ?? 0;
    const name = nameLen > 0 ? this.getString(u32[w] ?? 0, nameLen) : null;
    const attrCount = u32[w + 2] ?? 0;

    const attributes: (MdxJsxAttribute | MdxJsxExpressionAttribute)[] = [];
    for (let i = 0; i < attrCount; i++) {
      const base = w + 4 + i * 5;
      attributes.push(
        decodeMdxJsxAttr(
          this.#u8[base << 2] ?? 0,
          this.getString(u32[base + 1] ?? 0, u32[base + 2] ?? 0),
          this.getString(u32[base + 3] ?? 0, u32[base + 4] ?? 0),
        ),
      );
    }

    return { name, attributes };
  }
}
