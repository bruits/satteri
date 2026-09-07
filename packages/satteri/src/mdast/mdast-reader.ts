import type { Position } from "unist";
import type { AlignType } from "mdast";
import { ArenaReader } from "../arena-reader.js";
import {
  FIELD,
  KIND_MDAST,
  W_CHILDREN_COUNT,
  W_CHILDREN_START,
  W_DATA_LEN,
  W_DATA_OFFSET,
  W_PARENT,
  W_START_OFFSET,
} from "../generated/arena-layout.js";
import { decodeMdxJsxAttr } from "../mdx-attr.js";
import type {
  ArenaWire,
  BufferHeader,
  MdastNodeRaw,
  MdxJsxAttributeUnion,
  StringRefRaw,
} from "../types.js";
import { decodeColumnAlign } from "./column-align.js";
import { NodeTypeName } from "./generated/node-types.js";

export { NodeType, NodeTypeName } from "./generated/node-types.js";

export class MdastReader {
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
    const arena = new ArenaReader(buffer, KIND_MDAST);
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

  getNode(nodeId: number): MdastNodeRaw {
    const nodeCount = this.nodeCount;
    if (nodeId >= nodeCount) {
      throw new RangeError(`Node ID ${nodeId} out of range (count: ${nodeCount})`);
    }
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    const type = this.#u8[this.#nodesB + nodeId * this.#strideB + FIELD.node_type] ?? 0;
    return {
      id: u32[w] ?? 0,
      type,
      typeName: NodeTypeName[type] ?? `Unknown(${type})`,
      parent: u32[w + W_PARENT] ?? 0,
      position: this.getPosition(nodeId),
      childrenStart: u32[w + W_CHILDREN_START] ?? 0,
      childrenCount: u32[w + W_CHILDREN_COUNT] ?? 0,
      dataOffset: u32[w + W_DATA_OFFSET] ?? 0,
      dataLen: u32[w + W_DATA_LEN] ?? 0,
    };
  }

  /** Fast path: read only the type byte for a node. */
  getNodeType(nodeId: number): number {
    return this.#u8[this.#nodesB + nodeId * this.#strideB + FIELD.node_type] ?? 0;
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

  /** Fast path: read only the parent id for a node (0xffffffff at the root). */
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

  /** Push child node IDs directly onto a stack array (reverse order for depth-first). */
  pushChildIds(nodeId: number, stack: number[]): void {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    const childrenStart = u32[w + W_CHILDREN_START] ?? 0;
    const childrenCount = u32[w + W_CHILDREN_COUNT] ?? 0;
    const base = this.#childrenW + childrenStart;
    for (let i = childrenCount - 1; i >= 0; i--) {
      stack.push(u32[base + i] ?? 0);
    }
  }

  // Read fields directly to avoid allocating an intermediate view or reference per field.
  fieldU8(nodeId: number, offset: number, fallback: number): number {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    if (offset >= (u32[w + W_DATA_LEN] ?? 0)) return fallback;
    return this.#u8[this.#typeDataB + (u32[w + W_DATA_OFFSET] ?? 0) + offset] ?? fallback;
  }

  fieldU32(nodeId: number, offset: number, fallback: number): number {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    if (offset + 4 > (u32[w + W_DATA_LEN] ?? 0)) return fallback;
    return u32[(this.#typeDataB + (u32[w + W_DATA_OFFSET] ?? 0) + offset) >> 2] ?? fallback;
  }

  /** `""` when the field is absent or empty; callers map that to `null` for
   *  nullable fields, matching the Rust decoders' bounds checks. */
  fieldString(nodeId: number, offset: number): string {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    if (offset + 8 > (u32[w + W_DATA_LEN] ?? 0)) return "";
    const at = (this.#typeDataB + (u32[w + W_DATA_OFFSET] ?? 0) + offset) >> 2;
    return this.getString(u32[at] ?? 0, u32[at + 1] ?? 0);
  }

  getTypeData(nodeId: number): Uint8Array {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    const dataOffset = u32[w + W_DATA_OFFSET] ?? 0;
    const dataLen = u32[w + W_DATA_LEN] ?? 0;
    if (dataLen === 0) return new Uint8Array(0);
    const start = this.#typeDataB + dataOffset;
    return this.#u8.subarray(start, start + dataLen);
  }

  /** Read a StringRef (offset: u32 LE, len: u32 LE) from type data. */
  readStringRef(typeData: Uint8Array, byteOffset = 0): StringRefRaw {
    const at = (typeData.byteOffset - this.#u8.byteOffset + byteOffset) >> 2;
    return {
      offset: this.#u32[at] ?? 0,
      len: this.#u32[at + 1] ?? 0,
    };
  }

  /** Alignment bytes follow a u32 count: 0=none, 1=left, 2=right, 3=center. */
  getTableAlign(nodeId: number): AlignType[] {
    const data = this.getTypeData(nodeId);
    if (data.length < 4) return [];
    const count = this.#u32[(data.byteOffset - this.#u8.byteOffset) >> 2] ?? 0;
    const result: AlignType[] = [];
    for (let i = 0; i < count; i++) {
      result.push(decodeColumnAlign(data[4 + i] ?? 0));
    }
    return result;
  }

  /** A zero-length element name represents an MDX fragment. */
  getMdxJsxElementName(nodeId: number): string | null {
    const data = this.getTypeData(nodeId);
    const nameRef = this.readStringRef(data, 0);
    return nameRef.len > 0 ? this.getString(nameRef.offset, nameRef.len) : null;
  }

  /** Attribute kinds: 0=boolean, 1=literal, 2=expression, 3=spread. */
  getMdxJsxElementData(nodeId: number): {
    name: string | null;
    attributes: MdxJsxAttributeUnion[];
  } {
    const u32 = this.#u32;
    const nw = this.#nodesW + nodeId * this.#strideW;
    const dataLen = u32[nw + W_DATA_LEN] ?? 0;
    if (dataLen < 16) {
      return { name: this.getMdxJsxElementName(nodeId), attributes: [] };
    }
    const w = (this.#typeDataB + (u32[nw + W_DATA_OFFSET] ?? 0)) >> 2;
    const nameLen = u32[w + 1] ?? 0;
    const name = nameLen > 0 ? this.getString(u32[w] ?? 0, nameLen) : null;
    const attrCount = u32[w + 2] ?? 0;

    const attributes: MdxJsxAttributeUnion[] = [];
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

  getDirectiveData(nodeId: number): { name: string; attributes: Record<string, string> } {
    const u32 = this.#u32;
    const nw = this.#nodesW + nodeId * this.#strideW;
    if ((u32[nw + W_DATA_LEN] ?? 0) < 16) {
      return { name: "", attributes: {} };
    }
    const w = (this.#typeDataB + (u32[nw + W_DATA_OFFSET] ?? 0)) >> 2;
    const name = this.getString(u32[w] ?? 0, u32[w + 1] ?? 0);
    const attrCount = u32[w + 2] ?? 0;

    const attributes: Record<string, string> = {};
    for (let i = 0; i < attrCount; i++) {
      const base = w + 4 + i * 4;
      const key = this.getString(u32[base] ?? 0, u32[base + 1] ?? 0);
      attributes[key] = this.getString(u32[base + 2] ?? 0, u32[base + 3] ?? 0);
    }

    return { name, attributes };
  }

  /**
   * Walk the tree depth-first. Return false from visitor to skip children.
   */
  walk(visitor: (nodeId: number, nodeType: number) => boolean | void, rootId = 0): void {
    const stack: number[] = [rootId];
    while (stack.length > 0) {
      const nodeId = stack.pop() ?? 0;
      const nodeType = this.getNodeType(nodeId);
      const result = visitor(nodeId, nodeType);
      if (result !== false) {
        this.pushChildIds(nodeId, stack);
      }
    }
  }

  /** Walk depth-first with full node objects (slower, but convenient). */
  walkFull(visitor: (node: MdastNodeRaw) => boolean | void, rootId = 0): void {
    this.walk((nodeId) => visitor(this.getNode(nodeId)), rootId);
  }
}
