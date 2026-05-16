import type { MdastNodeRaw, BufferHeader, StringRefRaw, MdxJsxAttributeUnion } from "../types.js";

// Node type discriminant values (must match NodeType enum in node.rs)
export const NodeType = Object.freeze({
  Root: 0,
  Paragraph: 1,
  Heading: 2,
  ThematicBreak: 3,
  Blockquote: 4,
  List: 5,
  ListItem: 6,
  Html: 7,
  Code: 8,
  Definition: 9,
  Text: 10,
  Emphasis: 11,
  Strong: 12,
  InlineCode: 13,
  Break: 14,
  Link: 15,
  Image: 16,
  LinkReference: 17,
  ImageReference: 18,
  FootnoteDefinition: 19,
  FootnoteReference: 20,
  Table: 21,
  TableRow: 22,
  TableCell: 23,
  Delete: 24,
  Yaml: 25,
  Toml: 26,
  Math: 27,
  InlineMath: 28,
  // Directives
  ContainerDirective: 30,
  LeafDirective: 31,
  TextDirective: 32,
  // MDX
  MdxJsxFlowElement: 100,
  MdxJsxTextElement: 101,
  MdxFlowExpression: 102,
  MdxTextExpression: 103,
  MdxjsEsm: 104,
} as const);

type NodeTypeValue = (typeof NodeType)[keyof typeof NodeType];

// Reverse map: number → string name
export const NodeTypeName: Record<number, string> = Object.fromEntries(
  Object.entries(NodeType).map(([k, v]) => [v, k]),
);

// MdastNode field offsets within NODE_STRUCT_SIZE bytes.
// Must match MdastNode #[repr(C)] layout in node.rs:
//   id: u32          @ 0
//   node_type: u8    @ 4
//   _pad: [u8; 3]    @ 5
//   parent: u32      @ 8
//   start_offset: u32 @ 12
//   end_offset: u32  @ 16
//   start_line: u32  @ 20
//   start_column: u32 @ 24
//   end_line: u32    @ 28
//   end_column: u32  @ 32
//   children_start: u32 @ 36
//   children_count: u32 @ 40
//   data_offset: u32 @ 44
//   data_len: u32    @ 48
//   Total: 52 bytes
const FIELD = {
  id: 0,
  node_type: 4, // u8
  parent: 8,
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

// BufferHeader field offsets (all u32, little-endian):
//   magic: [u8; 4]        @ 0
//   kind: u32              @ 4   (1 = mdast, 2 = hast)
//   node_struct_size: u32  @ 8
//   node_count: u32        @ 12
//   nodes_offset: u32      @ 16
//   children_count: u32    @ 20
//   children_offset: u32   @ 24
//   type_data_len: u32     @ 28
//   type_data_offset: u32  @ 32
//   source_len: u32        @ 36
//   source_offset: u32     @ 40
//   Total: 44 bytes

const MAGIC = 0x5241444d; // "MDAR" bytes [0x4d,0x44,0x41,0x52] read as little-endian u32
const KIND_MDAST = 1;

export class MdastReader {
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
      throw new Error(
        `Invalid buffer: bad magic 0x${magic.toString(16)}, expected 0x${MAGIC.toString(16)}`,
      );
    }
    const kind = v.getUint32(4, true);
    if (kind !== KIND_MDAST) {
      throw new Error(
        `MdastReader was handed a buffer of kind ${kind} (expected ${KIND_MDAST}). ` +
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

  getNode(nodeId: number): MdastNodeRaw {
    const { nodesOffset, nodeStructSize, nodeCount } = this.#header;
    if (nodeId >= nodeCount) {
      throw new RangeError(`Node ID ${nodeId} out of range (count: ${nodeCount})`);
    }
    const base = nodesOffset + nodeId * nodeStructSize;
    const v = this.#view;
    const type = v.getUint8(base + FIELD.node_type);
    const startLine = v.getUint32(base + FIELD.start_line, true);
    const startOffset = v.getUint32(base + FIELD.start_offset, true);
    // Sentinel: nodes with both line=0 and offset=0 are synthesized
    // without source position (e.g. GFM autolink-literal nodes that
    // remark-gfm produces via mdast-util-find-and-replace).
    const position =
      startLine === 0 && startOffset === 0
        ? undefined
        : {
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

  getChildIds(nodeId: number): number[] {
    const node = this.getNode(nodeId);
    if (node.childrenCount === 0) return [];
    const { childrenOffset } = this.#header;
    const ids: number[] = [];
    for (let i = 0; i < node.childrenCount; i++) {
      const off = childrenOffset + (node.childrenStart + i) * 4;
      ids.push(this.#view.getUint32(off, true));
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
    const node = this.getNode(nodeId);
    if (node.dataLen === 0) return new Uint8Array(0);
    const { typeDataOffset } = this.#header;
    return new Uint8Array(
      this.#view.buffer,
      this.#view.byteOffset + typeDataOffset + node.dataOffset,
      node.dataLen,
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

  /** HeadingData: depth u8 @ 0. */
  getHeadingDepth(nodeId: number): number {
    return this.getTypeData(nodeId)[0]!;
  }

  /**
   * StringRef value. Valid for Text, InlineCode, Html, Yaml, Toml, InlineMath nodes.
   * These store a single StringRef as their type data.
   */
  getTextValue(nodeId: number): string {
    const data = this.getTypeData(nodeId);
    const ref = this.readStringRef(data);
    return this.getString(ref.offset, ref.len);
  }

  /**
   * LinkData: url(0..8), title(8..16).
   * Valid for Link nodes.
   */
  getLinkData(nodeId: number): { url: string; title: string | null } {
    const data = this.getTypeData(nodeId);
    const urlRef = this.readStringRef(data, 0);
    const titleRef = this.readStringRef(data, 8);
    return {
      url: this.getString(urlRef.offset, urlRef.len),
      title: titleRef.len > 0 ? this.getString(titleRef.offset, titleRef.len) : null,
    };
  }

  /**
   * ImageData: url(0..8), alt(8..16), title(16..24).
   * Valid for Image nodes.
   */
  getImageData(nodeId: number): { url: string; alt: string; title: string | null } {
    const data = this.getTypeData(nodeId);
    const urlRef = this.readStringRef(data, 0);
    const altRef = this.readStringRef(data, 8);
    const titleRef = this.readStringRef(data, 16);
    return {
      url: this.getString(urlRef.offset, urlRef.len),
      alt: this.getString(altRef.offset, altRef.len),
      title: titleRef.len > 0 ? this.getString(titleRef.offset, titleRef.len) : null,
    };
  }

  /**
   * CodeData #[repr(C)]: lang(0..8), meta(8..16), value(16..24), fence_char(24), _pad(25..28).
   * Valid for Code nodes.
   */
  getCodeData(nodeId: number): { lang: string | null; meta: string | null; value: string } {
    const data = this.getTypeData(nodeId);
    const langRef = this.readStringRef(data, 0);
    const metaRef = this.readStringRef(data, 8);
    const valueRef = this.readStringRef(data, 16);
    return {
      lang: langRef.len > 0 ? this.getString(langRef.offset, langRef.len) : null,
      meta: metaRef.len > 0 ? this.getString(metaRef.offset, metaRef.len) : null,
      value: this.getString(valueRef.offset, valueRef.len),
    };
  }

  /**
   * MathData #[repr(C)]: meta(0..8), value(8..16).
   * Valid for Math nodes.
   */
  getMathData(nodeId: number): { meta: string | null; value: string } {
    const data = this.getTypeData(nodeId);
    const metaRef = this.readStringRef(data, 0);
    const valueRef = this.readStringRef(data, 8);
    return {
      meta: metaRef.len > 0 ? this.getString(metaRef.offset, metaRef.len) : null,
      value: this.getString(valueRef.offset, valueRef.len),
    };
  }

  /**
   * DefinitionData #[repr(C)]: url(0..8), title(8..16), identifier(16..24), label(24..32).
   * Valid for Definition nodes.
   */
  getDefinitionData(nodeId: number): {
    url: string;
    title: string | null;
    identifier: string;
    label: string;
  } {
    const data = this.getTypeData(nodeId);
    const urlRef = this.readStringRef(data, 0);
    const titleRef = this.readStringRef(data, 8);
    const identifierRef = this.readStringRef(data, 16);
    const labelRef = this.readStringRef(data, 24);
    return {
      url: this.getString(urlRef.offset, urlRef.len),
      title: titleRef.len > 0 ? this.getString(titleRef.offset, titleRef.len) : null,
      identifier: this.getString(identifierRef.offset, identifierRef.len),
      label: this.getString(labelRef.offset, labelRef.len),
    };
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
   * ReferenceData #[repr(C)]: identifier(0..8), label(8..16), reference_kind(16), _pad(17..20).
   * referenceKind: 0=shortcut, 1=collapsed, 2=full.
   * Valid for LinkReference, ImageReference, FootnoteReference nodes.
   */
  getReferenceData(nodeId: number): { identifier: string; label: string; referenceType: string } {
    const data = this.getTypeData(nodeId);
    const identifierRef = this.readStringRef(data, 0);
    const labelRef = this.readStringRef(data, 8);
    const kindByte = data[16]!;
    const referenceTypes = ["shortcut", "collapsed", "full"];
    return {
      identifier: this.getString(identifierRef.offset, identifierRef.len),
      label: this.getString(labelRef.offset, labelRef.len),
      referenceType: referenceTypes[kindByte] ?? "shortcut",
    };
  }

  /**
   * ImageReference layout: 20-byte ReferenceData header + 8-byte alt StringRef.
   * Parser-emitted image references carry alt inline; plugin-created ones
   * may lack this suffix, in which case `alt` is empty.
   */
  getImageReferenceData(nodeId: number): {
    identifier: string;
    label: string;
    referenceType: string;
    alt: string;
  } {
    const base = this.getReferenceData(nodeId);
    const data = this.getTypeData(nodeId);
    if (data.length >= 28) {
      const altRef = this.readStringRef(data, 20);
      return {
        ...base,
        alt: this.getString(altRef.offset, altRef.len),
      };
    }
    return { ...base, alt: "" };
  }

  /**
   * FootnoteDefinitionData #[repr(C)]: identifier(0..8), label(8..16).
   */
  getFootnoteDefinitionData(nodeId: number): { identifier: string; label: string } {
    const data = this.getTypeData(nodeId);
    const identifierRef = this.readStringRef(data, 0);
    const labelRef = this.readStringRef(data, 8);
    return {
      identifier: this.getString(identifierRef.offset, identifierRef.len),
      label: this.getString(labelRef.offset, labelRef.len),
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
    const alignNames: (string | null)[] = [null, "left", "right", "center"];
    const result: (string | null)[] = [];
    for (let i = 0; i < count; i++) {
      result.push(alignNames[data[4 + i]!] ?? null);
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
              value: this.getString(attrValueRef.offset, attrValueRef.len),
            },
          });
          break;
        case 3: // Spread
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
   * ExpressionData #[repr(C)]: value StringRef (0..8).
   * Valid for MdxFlowExpression, MdxTextExpression, MdxjsEsm.
   */
  getExpressionValue(nodeId: number): string {
    const data = this.getTypeData(nodeId);
    const valueRef = this.readStringRef(data, 0);
    return this.getString(valueRef.offset, valueRef.len);
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
