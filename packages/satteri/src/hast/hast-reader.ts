import type { BufferHeader } from "../types.js";
import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "../mdx-types.js";

export type { MdxJsxAttribute, MdxJsxExpressionAttribute };

// HAST node type constants (must match node_types.rs)
export const HAST_ROOT = 0;
export const HAST_ELEMENT = 1;
export const HAST_TEXT = 2;
export const HAST_COMMENT = 3;
export const HAST_DOCTYPE = 4;
export const HAST_RAW = 5;

// MDX-specific HAST node types
export const HAST_MDX_JSX_ELEMENT = 10;
export const HAST_MDX_JSX_TEXT_ELEMENT = 11;
export const HAST_MDX_FLOW_EXPRESSION = 12;
export const HAST_MDX_ESM = 13;
export const HAST_MDX_TEXT_EXPRESSION = 14;

const PROP_STRING = 0;
const PROP_BOOL_TRUE = 1;
const PROP_BOOL_FALSE = 2;
const PROP_SPACE_SEP = 3;
const PROP_COMMA_SEP = 4;

// MDX JSX attribute kinds (must match node_types.rs)
const MDX_ATTR_BOOLEAN_PROP = 0;
const MDX_ATTR_LITERAL_PROP = 1;
const MDX_ATTR_EXPRESSION_PROP = 2;
const MDX_ATTR_SPREAD = 3;

export interface HastProperty {
  name: string;
  value: string | number | boolean | string[];
}

// HastNode field offsets (same layout as MDAST, shared binary format)
//   id: u32          @ 0
//   node_type: u8    @ 4
//   _pad: [u8; 3]    @ 5
//   parent: u32      @ 8
//   ...
//   children_start: u32 @ 36
//   children_count: u32 @ 40
//   data_offset: u32 @ 44
//   data_len: u32    @ 48
//   Total: 52 bytes
const FIELD = {
  node_type: 4,
  start_offset: 12,
  end_offset: 16,
  start_line: 20,
  start_column: 24,
  end_line: 28,
  end_column: 32,
  children_start: 36,
  children_count: 40,
  data_offset: 44,
  data_len: 48,
} as const;

// "MDAR" bytes: 4d 44 41 52; read as LE u32 = 0x5241444d
const MAGIC = 0x5241444d;
const KIND_HAST = 2;

export class HastReader {
  readonly #view: DataView;
  readonly #header: BufferHeader;
  readonly #textDecoder: TextDecoder;
  #sourceCache: string | null = null;

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
    const magic = v.getUint32(0, true);
    if (magic !== MAGIC) {
      throw new Error(`Invalid HAST buffer: bad magic 0x${magic.toString(16)}`);
    }
    const kind = v.getUint32(4, true);
    if (kind !== KIND_HAST) {
      throw new Error(
        `HastReader was handed a buffer of kind ${kind} (expected ${KIND_HAST}). ` +
          `MDAST and HAST node types overlap; reading the wrong kind decodes garbage.`,
      );
    }
    return {
      nodeStructSize: v.getUint32(8, true),
      nodeCount: v.getUint32(12, true),
      nodesOffset: v.getUint32(16, true),
      childrenCount: v.getUint32(20, true),
      childrenOffset: v.getUint32(24, true),
      typeDataLen: v.getUint32(28, true),
      typeDataOffset: v.getUint32(32, true),
      sourceLen: v.getUint32(36, true),
      sourceOffset: v.getUint32(40, true),
      nodeDataCount: v.getUint32(44, true),
      nodeDataOffset: v.getUint32(48, true),
    };
  }

  #nodeDataTable: Map<number, string> | null = null;

  /**
   * Per-node JSON `data` blob (set via `Arena::set_node_data` on the Rust
   * side). Returns `null` when the node has no entry. Lazy-builds a
   * `Map<id, string>` on first call so a materialization pass on a
   * data-heavy tree stays O(nodes) rather than O(nodes × entries).
   */
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

  /** Get the full source string (string pool). */
  getSource(): string {
    if (this.#sourceCache === null) {
      const { sourceOffset, sourceLen } = this.#header;
      const bytes = new Uint8Array(
        this.#view.buffer,
        this.#view.byteOffset + sourceOffset,
        sourceLen,
      );
      this.#sourceCache = this.#textDecoder.decode(bytes);
    }
    return this.#sourceCache;
  }

  /** Read a substring from the string pool by byte offset and length. */
  getString(offset: number, len: number): string {
    if (len === 0) return "";
    const { sourceOffset } = this.#header;
    const bytes = new Uint8Array(
      this.#view.buffer,
      this.#view.byteOffset + sourceOffset + offset,
      len,
    );
    return this.#textDecoder.decode(bytes);
  }

  /** Get position data for a node. */
  getPosition(nodeId: number):
    | {
        start: { offset: number; line: number; column: number };
        end: { offset: number; line: number; column: number };
      }
    | undefined {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    const startLine = v.getUint32(base + FIELD.start_line, true);
    const startOffset = v.getUint32(base + FIELD.start_offset, true);
    if (startLine === 0 && startOffset === 0) return undefined;
    return {
      start: {
        offset: startOffset,
        line: startLine,
        column: v.getUint32(base + FIELD.start_column, true),
      },
      end: {
        offset: v.getUint32(base + FIELD.end_offset, true),
        line: v.getUint32(base + FIELD.end_line, true),
        column: v.getUint32(base + FIELD.end_column, true),
      },
    };
  }

  /** Get the node_type byte for a given node ID. */
  getNodeType(nodeId: number): number {
    const { nodesOffset, nodeStructSize } = this.#header;
    return this.#view.getUint8(nodesOffset + nodeId * nodeStructSize + FIELD.node_type);
  }

  /** Get child node IDs for a given node. */
  getChildIds(nodeId: number): number[] {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    const childrenStart = v.getUint32(base + FIELD.children_start, true);
    const childrenCount = v.getUint32(base + FIELD.children_count, true);
    if (childrenCount === 0) return [];
    const { childrenOffset } = this.#header;
    const ids: number[] = [];
    for (let i = 0; i < childrenCount; i++) {
      ids.push(v.getUint32(childrenOffset + (childrenStart + i) * 4, true));
    }
    return ids;
  }

  /** Push child node IDs directly onto a stack array (reverse order for depth-first). */
  pushChildIds(nodeId: number, stack: number[]): void {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    const childrenStart = v.getUint32(base + FIELD.children_start, true);
    const childrenCount = v.getUint32(base + FIELD.children_count, true);
    if (childrenCount === 0) return;
    const { childrenOffset } = this.#header;
    for (let i = childrenCount - 1; i >= 0; i--) {
      stack.push(v.getUint32(childrenOffset + (childrenStart + i) * 4, true));
    }
  }

  /** Get the raw type_data bytes for a node. */
  getTypeData(nodeId: number): Uint8Array {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    const dataOffset = v.getUint32(base + FIELD.data_offset, true);
    const dataLen = v.getUint32(base + FIELD.data_len, true);
    if (dataLen === 0) return new Uint8Array(0);
    return new Uint8Array(
      this.#view.buffer,
      this.#view.byteOffset + this.#header.typeDataOffset + dataOffset,
      dataLen,
    );
  }

  /** Read a StringRef (offset: u32 LE, len: u32 LE) from a byte array at byteOffset. */
  #readStringRef(data: Uint8Array, byteOffset: number): { offset: number; len: number } {
    const view = new DataView(data.buffer, data.byteOffset + byteOffset);
    return {
      offset: view.getUint32(0, true),
      len: view.getUint32(4, true),
    };
  }

  /**
   * Get element data for a HAST_ELEMENT node.
   *
   * Element type_data layout:
   *   [tag_name: StringRef(8B)][prop_count: u32(4B)][_pad: u32(4B)] = 16-byte header
   *   then prop_count * 20 bytes:
   *     [name: StringRef(8B)][value_type: u8(1B)][_pad: [u8;3](3B)][value: StringRef(8B)]
   */
  getElementData(nodeId: number): { tagName: string; properties: HastProperty[] } {
    const data = this.getTypeData(nodeId);
    if (data.length < 16) {
      return { tagName: "", properties: [] };
    }

    const tagRef = this.#readStringRef(data, 0);
    const tagName = this.getString(tagRef.offset, tagRef.len);

    const view = new DataView(data.buffer, data.byteOffset + 8);
    const propCount = view.getUint32(0, true);

    const properties: HastProperty[] = [];
    for (let i = 0; i < propCount; i++) {
      const base = 16 + i * 20;
      const nameRef = this.#readStringRef(data, base);
      const name = this.getString(nameRef.offset, nameRef.len);
      const valueType = data[base + 8];
      const valueRef = this.#readStringRef(data, base + 12);

      switch (valueType) {
        case PROP_BOOL_TRUE:
          properties.push({ name, value: true });
          break;
        case PROP_BOOL_FALSE:
          properties.push({ name, value: false });
          break;
        case PROP_STRING:
          properties.push({ name, value: this.getString(valueRef.offset, valueRef.len) });
          break;
        case PROP_SPACE_SEP: {
          const raw = this.getString(valueRef.offset, valueRef.len);
          properties.push({ name, value: raw.split(" ").filter((s) => s.length > 0) });
          break;
        }
        case PROP_COMMA_SEP: {
          const raw = this.getString(valueRef.offset, valueRef.len);
          properties.push({
            name,
            value: raw
              .split(",")
              .map((s) => s.trim())
              .filter((s) => s.length > 0),
          });
          break;
        }
        case 5: {
          // PROP_INT
          const raw = this.getString(valueRef.offset, valueRef.len);
          properties.push({ name, value: Number(raw) });
          break;
        }
      }
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
    const data = this.getTypeData(nodeId);
    if (data.length < 16) {
      return { name: null, attributes: [] };
    }

    const nameRef = this.#readStringRef(data, 0);
    const name = nameRef.len > 0 ? this.getString(nameRef.offset, nameRef.len) : null;

    const view = new DataView(data.buffer, data.byteOffset + 8);
    const attrCount = view.getUint32(0, true);

    const attributes: (MdxJsxAttribute | MdxJsxExpressionAttribute)[] = [];
    for (let i = 0; i < attrCount; i++) {
      const base = 16 + i * 20;
      const kind = data[base]!;
      const attrNameRef = this.#readStringRef(data, base + 4);
      const attrValueRef = this.#readStringRef(data, base + 12);

      switch (kind) {
        case MDX_ATTR_BOOLEAN_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: null,
          });
          break;
        case MDX_ATTR_LITERAL_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: this.getString(attrValueRef.offset, attrValueRef.len),
          });
          break;
        case MDX_ATTR_EXPRESSION_PROP:
          attributes.push({
            type: "mdxJsxAttribute",
            name: this.getString(attrNameRef.offset, attrNameRef.len),
            value: {
              type: "mdxJsxAttributeValueExpression",
              value: this.getString(attrValueRef.offset, attrValueRef.len),
            },
          });
          break;
        case MDX_ATTR_SPREAD:
          attributes.push({
            type: "mdxJsxExpressionAttribute",
            value: this.getString(attrValueRef.offset, attrValueRef.len),
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
    const data = this.getTypeData(nodeId);
    if (data.length < 8) return "";
    const ref = this.#readStringRef(data, 0);
    return this.getString(ref.offset, ref.len);
  }
}
