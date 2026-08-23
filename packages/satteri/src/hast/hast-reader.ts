import type { ArenaWire, BufferHeader } from "../types.js";
import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "../mdx-types.js";
import { restorePhantomSpaces } from "../phantom.js";
import type { Position } from "unist";
import {
  MDX_ATTR_BOOLEAN_PROP,
  MDX_ATTR_LITERAL_PROP,
  MDX_ATTR_EXPRESSION_PROP,
  MDX_ATTR_SPREAD,
} from "../op-stream.js";
import { decodeElementProp } from "./element-props.js";
import { NAME_TO_TYPE } from "./generated/node-types.js";
import { ARENA_MAGIC, KIND_HAST, FIELD, HEADER } from "../generated/arena-layout.js";
import {
  W_CHILDREN_COUNT,
  W_CHILDREN_START,
  W_DATA_LEN,
  W_DATA_OFFSET,
  W_PARENT,
  W_START_OFFSET,
} from "../arena-words.js";

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
  readonly #view: DataView;
  readonly #header: BufferHeader;
  readonly #textDecoder: TextDecoder;
  // Typed-array views over the aligned LE wire: DataView getters cost far more in unoptimized V8 tiers.
  readonly #u8: Uint8Array;
  readonly #u32: Uint32Array;
  readonly #nodesB: number;
  readonly #nodesW: number;
  readonly #strideB: number;
  readonly #strideW: number;
  readonly #childrenW: number;
  readonly #typeDataB: number;
  readonly #nodeDataCount: number;
  #stringPoolCache: string | null = null;

  constructor(buffer: ArrayBuffer | Uint8Array) {
    let u8 = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
    // A u32 view needs a 4-byte-aligned base, which a foreign slice may not have.
    if ((u8.byteOffset & 3) !== 0) u8 = u8.slice();
    this.#u8 = u8;
    this.#view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
    this.#u32 = new Uint32Array(u8.buffer, u8.byteOffset, u8.byteLength >> 2);
    this.#textDecoder = new TextDecoder("utf-8", { ignoreBOM: true });
    const header = this.#readHeader();
    this.#header = header;
    this.#nodesB = header.nodesOffset;
    this.#nodesW = header.nodesOffset >> 2;
    this.#strideB = header.nodeStructSize;
    this.#strideW = header.nodeStructSize >> 2;
    this.#childrenW = header.childrenOffset >> 2;
    this.#typeDataB = header.typeDataOffset;
    this.#nodeDataCount = header.nodeDataCount;
  }

  #readHeader(): BufferHeader {
    const v = this.#view;
    const magic = v.getUint32(HEADER.magic, true);
    if (magic !== ARENA_MAGIC) {
      throw new Error(`Invalid HAST buffer: bad magic 0x${magic.toString(16)}`);
    }
    const kind = v.getUint32(HEADER.kind, true);
    if (kind !== KIND_HAST) {
      throw new Error(
        `HastReader was handed a buffer of kind ${kind} (expected ${KIND_HAST}). ` +
          `MDAST and HAST node types overlap; reading the wrong kind decodes garbage.`,
      );
    }
    return {
      nodeStructSize: v.getUint32(HEADER.node_struct_size, true),
      nodeCount: v.getUint32(HEADER.node_count, true),
      nodesOffset: v.getUint32(HEADER.nodes_offset, true),
      childrenCount: v.getUint32(HEADER.children_count, true),
      childrenOffset: v.getUint32(HEADER.children_offset, true),
      typeDataLen: v.getUint32(HEADER.type_data_len, true),
      typeDataOffset: v.getUint32(HEADER.type_data_offset, true),
      stringPoolLen: v.getUint32(HEADER.string_pool_len, true),
      stringPoolOffset: v.getUint32(HEADER.string_pool_offset, true),
      nodeDataCount: v.getUint32(HEADER.node_data_count, true),
      nodeDataOffset: v.getUint32(HEADER.node_data_offset, true),
    };
  }

  #nodeDataTable: Map<number, string> | null = null;

  /** @internal */
  getWire(): ArenaWire {
    return {
      u8: this.#u8,
      u32: this.#u32,
      nodesB: this.#nodesB,
      nodesW: this.#nodesW,
      strideB: this.#strideB,
      strideW: this.#strideW,
      childrenW: this.#childrenW,
      typeDataB: this.#typeDataB,
      pool: this.getStringPool(),
    };
  }

  /**
   * Per-node JSON `data` blob (set via `Arena::set_node_data` on the Rust
   * side). Returns `null` when the node has no entry. Lazy-builds a
   * `Map<id, string>` on first call so a materialization pass on a
   * data-heavy tree stays O(nodes) rather than O(nodes × entries).
   */
  getNodeData(nodeId: number): string | null {
    const table = this.getNodeDataTable();
    if (table === null) return null;
    return table.get(nodeId) ?? null;
  }

  /** @internal `null` when no node carries a `data` blob. */
  getNodeDataTable(): ReadonlyMap<number, string> | null {
    if (this.#nodeDataCount === 0) return null;
    if (this.#nodeDataTable === null) {
      this.#nodeDataTable = new Map();
      const v = this.#view;
      let pos = this.#header.nodeDataOffset;
      for (let i = 0; i < this.#nodeDataCount; i++) {
        const id = v.getUint32(pos, true);
        pos += 4;
        const len = v.getUint32(pos, true);
        pos += 4;
        const slice = this.#u8.subarray(pos, pos + len);
        this.#nodeDataTable.set(id, this.#textDecoder.decode(slice));
        pos += len;
      }
    }
    return this.#nodeDataTable;
  }

  get nodeCount(): number {
    return this.#header.nodeCount;
  }
  get header(): BufferHeader {
    return { ...this.#header };
  }

  /** The full string pool (original input + interning heap). Not the document
   * source as written; for that, read `ctx.source` from a plugin. */
  getStringPool(): string {
    return this.#stringPoolCache ?? this.#initPool();
  }

  #initPool(): string {
    const { stringPoolOffset, stringPoolLen } = this.#header;
    const pool = this.#textDecoder.decode(
      this.#u8.subarray(stringPoolOffset, stringPoolOffset + stringPoolLen),
    );
    this.#stringPoolCache = pool;
    return pool;
  }

  /** String refs arrive in UTF-16 units (the serializer remaps multibyte pools), so a plain substring is exact. */
  getString(offset: number, len: number): string {
    if (len === 0) return "";
    const pool = this.#stringPoolCache ?? this.#initPool();
    return pool.substring(offset, offset + len);
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

  #stringAt(at: number): string {
    const w = at >> 2;
    return this.getString(this.#u32[w] ?? 0, this.#u32[w + 1] ?? 0);
  }

  /** @internal Both element fields in one pass; the per-field accessors below
   *  each re-resolve the node's `type_data` position.
   *
   * Element type_data layout:
   *   [tag_name: StringRef(8B)][prop_count: u32(4B)][_pad: u32(4B)] = 16-byte header
   *   then prop_count * 20 bytes:
   *     [name: StringRef(8B)][value_type: u8(1B)][_pad: [u8;3](3B)][value: StringRef(8B)]
   */
  readElementInto(
    nodeId: number,
    node: { tagName: string; properties: Record<string, HastProperty["value"]> },
  ): void {
    const at = this.#typeDataAt(nodeId, 16);
    if (at === -1) {
      node.tagName = "";
      node.properties = {};
      return;
    }
    const u32 = this.#u32;
    const u8 = this.#u8;
    const w = at >> 2;
    node.tagName = this.getString(u32[w] ?? 0, u32[w + 1] ?? 0);
    const count = u32[w + 2] ?? 0;
    const properties: Record<string, HastProperty["value"]> = {};
    for (let i = 0; i < count; i++) {
      const base = w + 4 + i * 5;
      properties[this.getString(u32[base] ?? 0, u32[base + 1] ?? 0)] = decodeElementProp(
        u8[(base + 2) << 2] ?? 0,
        this.getString(u32[base + 3] ?? 0, u32[base + 4] ?? 0),
      );
    }
    node.properties = properties;
  }

  /** Element `tagName`, `properties` count, and the i-th property, read without
   *  building the intermediate views and records `getElementData` allocates. */
  getElementTagName(nodeId: number): string {
    const at = this.#typeDataAt(nodeId, 16);
    return at === -1 ? "" : this.#stringAt(at);
  }

  getElementPropCount(nodeId: number): number {
    const at = this.#typeDataAt(nodeId, 16);
    return at === -1 ? 0 : (this.#u32[(at >> 2) + 2] ?? 0);
  }

  getElementPropName(nodeId: number, index: number): string {
    const at = this.#typeDataAt(nodeId, 16);
    return at === -1 ? "" : this.#stringAt(at + 16 + index * 20);
  }

  getElementPropValue(nodeId: number, index: number): HastProperty["value"] {
    const at = this.#typeDataAt(nodeId, 16);
    if (at === -1) return "";
    const base = at + 16 + index * 20;
    return decodeElementProp(this.#u8[base + 8] ?? 0, this.#stringAt(base + 12));
  }

  getElementData(nodeId: number): { tagName: string; properties: HastProperty[] } {
    const at = this.#typeDataAt(nodeId, 16);
    if (at === -1) {
      return { tagName: "", properties: [] };
    }
    const u32 = this.#u32;
    const w = at >> 2;
    const tagName = this.getString(u32[w] ?? 0, u32[w + 1] ?? 0);
    const propCount = u32[w + 2] ?? 0;

    const properties: HastProperty[] = [];
    for (let i = 0; i < propCount; i++) {
      const base = w + 4 + i * 5;
      const name = this.getString(u32[base] ?? 0, u32[base + 1] ?? 0);
      const valueStr = this.getString(u32[base + 3] ?? 0, u32[base + 4] ?? 0);
      properties.push({ name, value: decodeElementProp(this.#u8[(base + 2) << 2] ?? 0, valueStr) });
    }

    return { tagName, properties };
  }

  /**
   * Get MDX JSX element data: name and attributes.
   *
   * MDX JSX element type_data layout:
   *   [name: StringRef(8B)][attr_count: u32(4B)][_pad: u32(4B)] = 16-byte header
   *   then attr_count * 20 bytes:
   *     [kind: u8(1B)][_pad: [u8;3](3B)][name: StringRef(8B)][value: StringRef(8B)]
   */
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
      const kind = this.#u8[base << 2] ?? 0;
      const attrName = () => this.getString(u32[base + 1] ?? 0, u32[base + 2] ?? 0);
      const attrValue = () => this.getString(u32[base + 3] ?? 0, u32[base + 4] ?? 0);

      switch (kind) {
        case MDX_ATTR_BOOLEAN_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: null,
          });
          break;
        case MDX_ATTR_LITERAL_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: attrValue(),
          });
          break;
        case MDX_ATTR_EXPRESSION_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: {
              type: "mdxJsxAttributeValueExpression",
              value: restorePhantomSpaces(attrValue()),
            },
          });
          break;
        case MDX_ATTR_SPREAD:
          attributes.push({
            type: "mdxJsxExpressionAttribute",
            value: restorePhantomSpaces(attrValue()),
          });
          break;
      }
    }

    return { name, attributes };
  }

  /**
   * Get the string value for HAST_TEXT, HAST_COMMENT, or HAST_RAW nodes.
   * These store a single StringRef (8 bytes) as their type_data.
   */
  getTextValue(nodeId: number): string {
    const u32 = this.#u32;
    const w = this.#nodesW + nodeId * this.#strideW;
    if ((u32[w + W_DATA_LEN] ?? 0) < 8) return "";
    const at = (this.#typeDataB + (u32[w + W_DATA_OFFSET] ?? 0)) >> 2;
    return this.getString(u32[at] ?? 0, u32[at + 1] ?? 0);
  }
}
