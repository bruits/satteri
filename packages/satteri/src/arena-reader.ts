import { ARENA_MAGIC, HEADER, KIND_MDAST } from "./generated/arena-layout.js";
import type { ArenaWire, BufferHeader } from "./types.js";

export class ArenaReader {
  readonly u8: Uint8Array;
  readonly u32: Uint32Array;
  readonly #view: DataView;
  readonly header: BufferHeader;
  readonly #textDecoder = new TextDecoder("utf-8", { ignoreBOM: true });
  #stringPoolCache: string | null = null;

  constructor(buffer: ArrayBuffer | Uint8Array, kind: number) {
    let u8 = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
    // A foreign slice may not have the alignment required by a u32 view.
    if ((u8.byteOffset & 3) !== 0) u8 = u8.slice();
    this.u8 = u8;
    this.u32 = new Uint32Array(u8.buffer, u8.byteOffset, u8.byteLength >> 2);
    this.#view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
    this.header = readHeader(this.#view, kind);
  }

  #nodeDataTable: Map<number, string> | null = null;

  #wire: ArenaWire | null = null;

  // Memoize wire setup because the lazy materializer requests it for every node.
  getWire(): ArenaWire {
    return (this.#wire ??= {
      u8: this.u8,
      u32: this.u32,
      nodesB: this.header.nodesOffset,
      nodesW: this.header.nodesOffset >> 2,
      strideB: this.header.nodeStructSize,
      strideW: this.header.nodeStructSize >> 2,
      childrenW: this.header.childrenOffset >> 2,
      typeDataB: this.header.typeDataOffset,
      pool: this.getStringPool(),
    });
  }

  // Index data blobs once to keep materialization O(nodes), rather than O(nodes × entries).
  getNodeData(nodeId: number): string | null {
    const table = this.getNodeDataTable();
    if (table === null) return null;
    return table.get(nodeId) ?? null;
  }

  getNodeDataTable(): ReadonlyMap<number, string> | null {
    if (this.header.nodeDataCount === 0) return null;
    if (this.#nodeDataTable === null) {
      this.#nodeDataTable = new Map();
      const v = this.#view;
      let pos = this.header.nodeDataOffset;
      for (let i = 0; i < this.header.nodeDataCount; i++) {
        const id = v.getUint32(pos, true);
        pos += 4;
        const len = v.getUint32(pos, true);
        pos += 4;
        const slice = this.u8.subarray(pos, pos + len);
        this.#nodeDataTable.set(id, this.#textDecoder.decode(slice));
        pos += len;
      }
    }
    return this.#nodeDataTable;
  }

  // The pool includes interned strings; it is not the original document source.
  getStringPool(): string {
    return this.#stringPoolCache ?? this.#initPool();
  }

  #initPool(): string {
    const { stringPoolOffset, stringPoolLen } = this.header;
    const pool = this.#textDecoder.decode(
      this.u8.subarray(stringPoolOffset, stringPoolOffset + stringPoolLen),
    );
    this.#stringPoolCache = pool;
    return pool;
  }

  // The serializer remaps string refs to UTF-16 offsets, matching substring indices.
  getString(offset: number, len: number): string {
    if (len === 0) return "";
    const pool = this.#stringPoolCache ?? this.#initPool();
    return pool.substring(offset, offset + len);
  }
}

function readHeader(view: DataView, expectedKind: number): BufferHeader {
  const magic = view.getUint32(HEADER.magic, true);
  if (magic !== ARENA_MAGIC) {
    throw new Error(
      expectedKind === KIND_MDAST
        ? `Invalid buffer: bad magic 0x${magic.toString(16)}, expected 0x${ARENA_MAGIC.toString(16)}`
        : `Invalid HAST buffer: bad magic 0x${magic.toString(16)}`,
    );
  }
  const kind = view.getUint32(HEADER.kind, true);
  if (kind !== expectedKind) {
    throw new Error(
      `${expectedKind === KIND_MDAST ? "MdastReader" : "HastReader"} was handed a buffer of kind ${kind} (expected ${expectedKind}). ` +
        `MDAST and HAST node types overlap; reading the wrong kind decodes garbage.`,
    );
  }
  return {
    nodeStructSize: view.getUint32(HEADER.node_struct_size, true),
    nodeCount: view.getUint32(HEADER.node_count, true),
    nodesOffset: view.getUint32(HEADER.nodes_offset, true),
    childrenCount: view.getUint32(HEADER.children_count, true),
    childrenOffset: view.getUint32(HEADER.children_offset, true),
    typeDataLen: view.getUint32(HEADER.type_data_len, true),
    typeDataOffset: view.getUint32(HEADER.type_data_offset, true),
    stringPoolLen: view.getUint32(HEADER.string_pool_len, true),
    stringPoolOffset: view.getUint32(HEADER.string_pool_offset, true),
    nodeDataCount: view.getUint32(HEADER.node_data_count, true),
    nodeDataOffset: view.getUint32(HEADER.node_data_offset, true),
  };
}
