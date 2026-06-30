import type { BufferHeader } from "../types.js";
import type { MdxJsxAttribute, MdxJsxExpressionAttribute } from "../mdx-types.js";
import { restorePhantomSpaces } from "../phantom.js";
import { readPosition } from "../wire-read.js";
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

export type { MdxJsxAttribute, MdxJsxExpressionAttribute };

export const HAST_ROOT = NAME_TO_TYPE.root!;
export const HAST_ELEMENT = NAME_TO_TYPE.element!;
export const HAST_TEXT = NAME_TO_TYPE.text!;
export const HAST_COMMENT = NAME_TO_TYPE.comment!;
export const HAST_DOCTYPE = NAME_TO_TYPE.doctype!;
export const HAST_RAW = NAME_TO_TYPE.raw!;
export const HAST_MDX_JSX_ELEMENT = NAME_TO_TYPE.mdxJsxFlowElement!;
export const HAST_MDX_JSX_TEXT_ELEMENT = NAME_TO_TYPE.mdxJsxTextElement!;
export const HAST_MDX_FLOW_EXPRESSION = NAME_TO_TYPE.mdxFlowExpression!;
export const HAST_MDX_ESM = NAME_TO_TYPE.mdxjsEsm!;
export const HAST_MDX_TEXT_EXPRESSION = NAME_TO_TYPE.mdxTextExpression!;

export interface HastProperty {
  name: string;
  value: string | number | boolean | string[];
}

export class HastReader {
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

  /** Read a substring from the string pool by byte offset and length. */
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

  /** Get position data for a node. */
  getPosition(nodeId: number): Position | undefined {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    return readPosition(this.#view, base + FIELD.start_offset);
  }

  /** Whether the node carries a source position (line 0 + offset 0 is the
   *  synthesized-node sentinel), without decoding the three position objects. */
  hasPosition(nodeId: number): boolean {
    const base = this.#header.nodesOffset + nodeId * this.#header.nodeStructSize;
    const v = this.#view;
    return (
      v.getUint32(base + FIELD.start_line, true) !== 0 ||
      v.getUint32(base + FIELD.start_offset, true) !== 0
    );
  }

  /** Get the node_type byte for a given node ID. */
  getNodeType(nodeId: number): number {
    const { nodesOffset, nodeStructSize } = this.#header;
    return this.#view.getUint8(nodesOffset + nodeId * nodeStructSize + FIELD.node_type);
  }

  /** Get the parent id for a given node (0xffffffff at the root). */
  getParentId(nodeId: number): number {
    const { nodesOffset, nodeStructSize } = this.#header;
    return this.#view.getUint32(nodesOffset + nodeId * nodeStructSize + FIELD.parent, true);
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
      const valueStr = this.getString(valueRef.offset, valueRef.len);
      properties.push({ name, value: decodeElementProp(valueType!, valueStr) });
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
              value: restorePhantomSpaces(this.getString(attrValueRef.offset, attrValueRef.len)),
            },
          });
          break;
        case MDX_ATTR_SPREAD:
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
