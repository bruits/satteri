import type { MdastNodeRaw, BufferHeader, StringRefRaw, MdxJsxAttributeUnion } from "../types.js";
import { restorePhantomSpaces } from "../phantom.js";
import { readPosition } from "../wire-read.js";
import { decodeColumnAlign } from "./column-align.js";
import { NodeTypeName } from "./generated/node-types.js";
import { ARENA_MAGIC, KIND_MDAST, FIELD, HEADER } from "../generated/arena-layout.js";

export { NodeType, NodeTypeName } from "./generated/node-types.js";

export class MdastReader {
  readonly #view: DataView;
  readonly #header: BufferHeader;
  readonly #textDecoder: TextDecoder;
  #stringPoolCache: string | null = null;

  constructor(buffer: ArrayBuffer | Uint8Array) {
    if (buffer instanceof Uint8Array) {
      this.#view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
    } else {
      this.#view = new DataView(buffer);
    }
    this.#textDecoder = new TextDecoder("utf-8");
    this.#header = this.#readHeader();
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

  /** Per-node JSON `data` blob (set via `Arena::set_node_data` on the Rust
   * side). Lazy-builds a `Map<id, string>` on first call so materialization
   * of a data-heavy tree stays O(nodes) rather than O(nodes × entries). */
  getNodeData(nodeId: number): string | null {
    if (this.#header.nodeDataCount === 0) return null;
    if (this.#nodeDataTable === null) {
      this.#nodeDataTable = new Map();
      const v = this.#view;
      let pos = this.#header.nodeDataOffset;
      for (let i = 0; i < this.#header.nodeDataCount; i++) {
        const id = v.getUint32(pos, true);
        pos += 4;
        const len = v.getUint32(pos, true);
        pos += 4;
        const slice = new Uint8Array(this.#view.buffer, this.#view.byteOffset + pos, len);
        this.#nodeDataTable.set(id, this.#textDecoder.decode(slice));
        pos += len;
      }
    }
    return this.#nodeDataTable.get(nodeId) ?? null;
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
    if (this.#stringPoolCache === null) {
      const { stringPoolOffset, stringPoolLen } = this.#header;
      const bytes = new Uint8Array(
        this.#view.buffer,
        this.#view.byteOffset + stringPoolOffset,
        stringPoolLen,
      );
      this.#stringPoolCache = this.#textDecoder.decode(bytes);
    }
    return this.#stringPoolCache;
  }

  getString(offset: number, len: number): string {
    if (len === 0) return "";
    const { stringPoolOffset } = this.#header;
    const bytes = new Uint8Array(
      this.#view.buffer,
      this.#view.byteOffset + stringPoolOffset + offset,
      len,
    );
    return this.#textDecoder.decode(bytes);
  }

  getNode(nodeId: number): MdastNodeRaw {
    const { nodesOffset, nodeStructSize, nodeCount } = this.#header;
    if (nodeId >= nodeCount) {
      throw new RangeError(`Node ID ${nodeId} out of range (count: ${nodeCount})`);
    }
    const base = nodesOffset + nodeId * nodeStructSize;
    const v = this.#view;
    const type = v.getUint8(base + FIELD.node_type);
    const position = readPosition(v, base + FIELD.start_offset);
    return {
      id: v.getUint32(base + FIELD.id, true),
      type,
      typeName: NodeTypeName[type] ?? `Unknown(${type})`,
      parent: v.getUint32(base + FIELD.parent, true),
      position,
      childrenStart: v.getUint32(base + FIELD.children_start, true),
      childrenCount: v.getUint32(base + FIELD.children_count, true),
      dataOffset: v.getUint32(base + FIELD.data_offset, true),
      dataLen: v.getUint32(base + FIELD.data_len, true),
    };
  }

  /** Fast path: read only the type byte for a node. */
  getNodeType(nodeId: number): number {
    const { nodesOffset, nodeStructSize } = this.#header;
    return this.#view.getUint8(nodesOffset + nodeId * nodeStructSize + FIELD.node_type);
  }

  /** Fast path: read only the parent id for a node (0xffffffff at the root). */
  getParentId(nodeId: number): number {
    const { nodesOffset, nodeStructSize } = this.#header;
    return this.#view.getUint32(nodesOffset + nodeId * nodeStructSize + FIELD.parent, true);
  }

  getChildIds(nodeId: number): number[] {
    const { nodesOffset, nodeStructSize, childrenOffset } = this.#header;
    const base = nodesOffset + nodeId * nodeStructSize;
    const v = this.#view;
    const childrenStart = v.getUint32(base + FIELD.children_start, true);
    const childrenCount = v.getUint32(base + FIELD.children_count, true);
    if (childrenCount === 0) return [];
    const ids: number[] = [];
    for (let i = 0; i < childrenCount; i++) {
      ids.push(v.getUint32(childrenOffset + (childrenStart + i) * 4, true));
    }
    return ids;
  }

  /** Push child node IDs directly onto a stack array (reverse order for depth-first). */
  pushChildIds(nodeId: number, stack: number[]): void {
    const { nodesOffset, nodeStructSize, childrenOffset } = this.#header;
    const base = nodesOffset + nodeId * nodeStructSize;
    const v = this.#view;
    const childrenStart = v.getUint32(base + FIELD.children_start, true);
    const childrenCount = v.getUint32(base + FIELD.children_count, true);
    if (childrenCount === 0) return;
    for (let i = childrenCount - 1; i >= 0; i--) {
      stack.push(v.getUint32(childrenOffset + (childrenStart + i) * 4, true));
    }
  }

  getTypeData(nodeId: number): Uint8Array {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    const dataOffset = v.getUint32(base + FIELD.data_offset, true);
    const dataLen = v.getUint32(base + FIELD.data_len, true);
    if (dataLen === 0) return new Uint8Array(0);
    return new Uint8Array(
      v.buffer,
      v.byteOffset + this.#header.typeDataOffset + dataOffset,
      dataLen,
    );
  }

  /** Read a StringRef (offset: u32 LE, len: u32 LE) from type data. */
  readStringRef(typeData: Uint8Array, byteOffset = 0): StringRefRaw {
    const view = new DataView(typeData.buffer, typeData.byteOffset + byteOffset);
    return {
      offset: view.getUint32(0, true),
      len: view.getUint32(4, true),
    };
  }

  /**
   * StringRef value. Valid for Text, InlineCode, Html, Yaml, Toml nodes.
   * These store a single StringRef as their type data.
   */
  getTextValue(nodeId: number): string {
    const data = this.getTypeData(nodeId);
    const ref = this.readStringRef(data);
    return this.getString(ref.offset, ref.len);
  }

  /**
   * ListData #[repr(C)]: start(0..4), ordered(4), spread(5), _pad(6..8).
   * Valid for List nodes.
   */
  getListData(nodeId: number): { ordered: boolean; start: number; spread: boolean } {
    const data = this.getTypeData(nodeId);
    const view = new DataView(data.buffer, data.byteOffset);
    return {
      start: view.getUint32(0, true),
      ordered: data[4] !== 0,
      spread: data[5] !== 0,
    };
  }

  /**
   * ListItemData #[repr(C)]: checked(0), spread(1).
   * checked: 0=unchecked, 1=checked, 2=not-a-task-item.
   */
  getListItemData(nodeId: number): { checked: boolean | null; spread: boolean } {
    const data = this.getTypeData(nodeId);
    const checkedByte = data[0];
    return {
      checked: checkedByte === 2 ? null : checkedByte === 1,
      spread: data[1] !== 0,
    };
  }

  /**
   * TableData #[repr(C)]: align_count(0..4), then align_count bytes.
   * Alignment bytes: 0=none, 1=left, 2=right, 3=center.
   */
  getTableAlign(nodeId: number): (string | null)[] {
    const data = this.getTypeData(nodeId);
    if (data.length < 4) return [];
    const view = new DataView(data.buffer, data.byteOffset);
    const count = view.getUint32(0, true);
    const result: (string | null)[] = [];
    for (let i = 0; i < count; i++) {
      result.push(decodeColumnAlign(data[4 + i]!));
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
    const data = this.getTypeData(nodeId);
    if (data.length < 16) {
      return { name: this.getMdxJsxElementName(nodeId), attributes: [] };
    }

    const nameRef = this.readStringRef(data, 0);
    const name = nameRef.len > 0 ? this.getString(nameRef.offset, nameRef.len) : null;

    const view = new DataView(data.buffer, data.byteOffset + 8);
    const attrCount = view.getUint32(0, true);

    const attributes: MdxJsxAttributeUnion[] = [];
    for (let i = 0; i < attrCount; i++) {
      const base = 16 + i * 20;
      const kind = data[base]!;
      const attrNameRef = this.readStringRef(data, base + 4);
      const attrValueRef = this.readStringRef(data, base + 12);

      switch (kind) {
        case 0: // BooleanProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: null,
          });
          break;
        case 1: // LiteralProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: this.getString(attrValueRef.offset, attrValueRef.len),
          });
          break;
        case 2: // ExpressionProp
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: {
              type: "mdxJsxAttributeValueExpression",
              value: restorePhantomSpaces(this.getString(attrValueRef.offset, attrValueRef.len)),
            },
          });
          break;
        case 3: // Spread
          attributes.push({
            type: "mdxJsxExpressionAttribute",
            value: restorePhantomSpaces(this.getString(attrValueRef.offset, attrValueRef.len)),
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
    const data = this.getTypeData(nodeId);
    if (data.length < 16) {
      return { name: "", attributes: {} };
    }

    const nameRef = this.readStringRef(data, 0);
    const name = this.getString(nameRef.offset, nameRef.len);

    const view = new DataView(data.buffer, data.byteOffset + 8);
    const attrCount = view.getUint32(0, true);

    const attributes: Record<string, string> = {};
    for (let i = 0; i < attrCount; i++) {
      const base = 16 + i * 16;
      const keyRef = this.readStringRef(data, base);
      const valRef = this.readStringRef(data, base + 8);
      const key = this.getString(keyRef.offset, keyRef.len);
      const val = this.getString(valRef.offset, valRef.len);
      attributes[key] = val;
    }

    return { name, attributes };
  }

  /**
   * Walk the tree depth-first. Return false from visitor to skip children.
   */
  walk(visitor: (nodeId: number, nodeType: number) => boolean | void, rootId = 0): void {
    const stack: number[] = [rootId];
    while (stack.length > 0) {
      const nodeId = stack.pop()!;
      const nodeType = this.getNodeType(nodeId);
      const result = visitor(nodeId, nodeType);
      if (result !== false) {
        const childIds = this.getChildIds(nodeId);
        for (let i = childIds.length - 1; i >= 0; i--) {
          stack.push(childIds[i]!);
        }
      }
    }
  }

  /** Walk depth-first with full node objects (slower, but convenient). */
  walkFull(visitor: (node: MdastNodeRaw) => boolean | void, rootId = 0): void {
    this.walk((nodeId) => visitor(this.getNode(nodeId)), rootId);
  }
}
