import type {
  ArenaWire,
  MdastNodeRaw,
  BufferHeader,
  StringRefRaw,
  MdxJsxAttributeUnion,
} from "../types.js";
import { restorePhantomSpaces } from "../phantom.js";
import { decodeColumnAlign } from "./column-align.js";
import { NodeTypeName } from "./generated/node-types.js";
import {
  ARENA_MAGIC,
  KIND_MDAST,
  FIELD,
  HEADER,
  W_CHILDREN_COUNT,
  W_CHILDREN_START,
  W_DATA_LEN,
  W_DATA_OFFSET,
  W_PARENT,
  W_START_OFFSET,
} from "../generated/arena-layout.js";
import type { Position } from "unist";

export { NodeType, NodeTypeName } from "./generated/node-types.js";

export class MdastReader {
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
      throw new Error(
        `Invalid buffer: bad magic 0x${magic.toString(16)}, expected 0x${ARENA_MAGIC.toString(16)}`,
      );
    }
    const kind = v.getUint32(HEADER.kind, true);
    if (kind !== KIND_MDAST) {
      throw new Error(
        `MdastReader was handed a buffer of kind ${kind} (expected ${KIND_MDAST}). ` +
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

  /** Per-node JSON `data` blob (set via `Arena::set_node_data` on the Rust
   * side). Returns `null` when the node has no entry. Lazy-builds a
   * `Map<id, string>` on first call so materialization of a data-heavy tree
   * stays O(nodes) rather than O(nodes × entries). */
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

  getNode(nodeId: number): MdastNodeRaw {
    const { nodeCount } = this.#header;
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

  /** Fixed-layout field reads straight off the u32/u8 views. The generated
   *  decoder runs once per node, so an intermediate view or ref object here
   *  costs an allocation per field. */
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

  /**
   * StringRef value. Valid for Text, InlineCode, Html, Yaml, Toml nodes.
   * These store a single StringRef as their type data.
   */
  getTextValue(nodeId: number): string {
    return this.fieldString(nodeId, 0);
  }

  /**
   * ListData #[repr(C)]: start(0..4), ordered(4), spread(5), _pad(6..8).
   * Valid for List nodes.
   */
  getListData(nodeId: number): { ordered: boolean; start: number; spread: boolean } {
    return {
      start: this.fieldU32(nodeId, 0, 0),
      ordered: this.fieldU8(nodeId, 4, 0) !== 0,
      spread: this.fieldU8(nodeId, 5, 0) !== 0,
    };
  }

  /**
   * ListItemData #[repr(C)]: checked(0), spread(1).
   * checked: 0=unchecked, 1=checked, 2=not-a-task-item.
   */
  getListItemData(nodeId: number): { checked: boolean | null; spread: boolean } {
    const checked = this.fieldU8(nodeId, 0, 2);
    return {
      checked: checked === 2 ? null : checked === 1,
      spread: this.fieldU8(nodeId, 1, 0) !== 0,
    };
  }

  /**
   * DescriptionDetailsData #[repr(C)]: spread(0). Valid for descriptionDetails.
   */
  getDescriptionDetailsData(nodeId: number): { spread: boolean } {
    return { spread: this.fieldU8(nodeId, 0, 0) !== 0 };
  }

  /**
   * TableData #[repr(C)]: align_count(0..4), then align_count bytes.
   * Alignment bytes: 0=none, 1=left, 2=right, 3=center.
   */
  getTableAlign(nodeId: number): (string | null)[] {
    const data = this.getTypeData(nodeId);
    if (data.length < 4) return [];
    const count = this.#u32[(data.byteOffset - this.#u8.byteOffset) >> 2] ?? 0;
    const result: (string | null)[] = [];
    for (let i = 0; i < count; i++) {
      result.push(decodeColumnAlign(data[4 + i] ?? 0));
    }
    return result;
  }

  /**
   * MdxJsxElementData: name StringRef (0..8). len===0 means fragment.
   */
  getMdxJsxElementName(nodeId: number): string | null {
    const data = this.getTypeData(nodeId);
    const nameRef = this.readStringRef(data, 0);
    return nameRef.len > 0 ? this.getString(nameRef.offset, nameRef.len) : null;
  }

  /**
   * MDX JSX element data: name + attributes.
   *
   * Layout:
   *   [name: StringRef(8B)][attr_count: u32(4B)][_pad: u32(4B)] = 16-byte header
   *   then attr_count * 20 bytes:
   *     [kind: u8(1B)][_pad: [u8;3](3B)][name: StringRef(8B)][value: StringRef(8B)]
   *
   * Attribute kinds: 0=boolean, 1=literal, 2=expression, 3=spread
   */
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
      const kind = this.#u8[base << 2] ?? 0;
      const attrName = () => this.getString(u32[base + 1] ?? 0, u32[base + 2] ?? 0);
      const attrValue = () => this.getString(u32[base + 3] ?? 0, u32[base + 4] ?? 0);

      switch (kind) {
        case 0: // BooleanProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: null,
          });
          break;
        case 1: // LiteralProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: attrValue(),
          });
          break;
        case 2: // ExpressionProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: attrName(),
            value: {
              type: "mdxJsxAttributeValueExpression",
              value: restorePhantomSpaces(attrValue()),
            },
          });
          break;
        case 3: // Spread
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
   * DirectiveData layout:
   *   [name: StringRef(8B)][attr_count: u32(4B)][_pad: u32(4B)] = 16-byte header
   *   then attr_count × 16 bytes:
   *     [key: StringRef(8B)][value: StringRef(8B)]
   */
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
