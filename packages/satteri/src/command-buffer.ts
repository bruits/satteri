/**
 * Binary command buffer for efficient JS→Rust mutation serialization.
 *
 * Simple mutations (remove, setProperty) are encoded as compact binary commands.
 * Structural mutations (insert, replace, …) carry payloads that are either raw
 * strings for Rust to re-parse (`{ raw }` markdown / `{ rawHtml }`) or, for
 * plugin-built node trees, a compact binary node encoding (no JSON, no repeated
 * field names) — see {@link encodeNodeTree}.
 *
 * All multi-byte integers are little-endian to match native x86/ARM layout and
 * avoid byte-swapping on the Rust side.
 */

// Command bytes (0x01–0x0F)

const CMD_REMOVE = 0x01;
const CMD_INSERT_BEFORE = 0x05;
const CMD_INSERT_AFTER = 0x06;
const CMD_PREPEND_CHILD = 0x07;
const CMD_APPEND_CHILD = 0x08;
const CMD_WRAP = 0x09;
const CMD_REPLACE = 0x0b;
const CMD_SET_PROPERTY = 0x0c;

// Payload types (0x10+, distinct range from commands)

const PAYLOAD_RAW_MARKDOWN = 0x10;
const PAYLOAD_RAW_HTML = 0x11;
const PAYLOAD_BINARY_NODE = 0x13;

// Value types for CMD_SET_PROPERTY (must match commands.rs PROP_* constants)

const PROP_STRING = 0;
const PROP_BOOL_TRUE = 1;
const PROP_BOOL_FALSE = 2;
const PROP_SPACE_SEP = 3;
const PROP_INT = 5;
const PROP_NULL = 6;

/** Resolve a node's arena id when it is an existing (reused) node, else
 *  `undefined` for a freshly-built one. Supplied by each visitor (the id maps
 *  live there). */
export type ReusedId = (node: unknown) => number | undefined;

type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

export function classifyReturn(value: unknown): ReturnClass {
  if (value === undefined || value === null) return "no_change";
  const v = value as Record<string, unknown>;
  if (typeof v.raw === "string") return "raw_markdown";
  if (typeof v.rawHtml === "string") return "raw_html";
  if (typeof v.type === "string") return "structured_node";
  throw new Error("Invalid return value from visitor: must have raw, rawHtml, or type");
}

const INITIAL_SIZE = 4096;
const encoder = new TextEncoder();
const EMPTY_U8 = new Uint8Array(0);

// Binary node payload — field bits (must match read_binary_node in js_commands.rs)

const F_VALUE = 0;
const F_DEPTH = 1;
const F_URL = 2;
const F_TITLE = 3;
const F_ALT = 4;
const F_LANG = 5;
const F_META = 6;
const F_ORDERED = 7;
const F_START = 8;
const F_SPREAD = 9;
const F_CHECKED = 10;
const F_IDENTIFIER = 11;
const F_LABEL = 12;
const F_REFERENCE_TYPE = 13;
const F_NAME = 14;
const F_TAG_NAME = 15;
const F_ATTRIBUTES = 16;
const F_PROPERTIES = 17;
const F_DATA = 18;
const F_KEEP_CHILDREN = 19;

/** Growable little-endian writer used to encode one node tree at a time. */
class NodeWriter {
  private buf = new Uint8Array(512);
  private view = new DataView(this.buf.buffer);
  private off = 0;

  reset(): void {
    this.off = 0;
  }

  private ensure(n: number): void {
    if (this.off + n <= this.buf.byteLength) return;
    let size = this.buf.byteLength * 2;
    while (this.off + n > size) size *= 2;
    const grown = new Uint8Array(size);
    grown.set(this.buf);
    this.buf = grown;
    this.view = new DataView(grown.buffer);
  }

  u8(v: number): void {
    this.ensure(1);
    this.buf[this.off++] = v & 0xff;
  }

  /** Unsigned LEB128 — 1 byte for values < 128 (the common case). */
  varint(v: number): void {
    this.ensure(5);
    let n = v >>> 0;
    while (n > 0x7f) {
      this.buf[this.off++] = (n & 0x7f) | 0x80;
      n >>>= 7;
    }
    this.buf[this.off++] = n;
  }

  str(s: string): void {
    this.bytes(encoder.encode(s));
  }

  bytes(b: Uint8Array): void {
    this.varint(b.length);
    this.ensure(b.length);
    this.buf.set(b, this.off);
    this.off += b.length;
  }

  take(): Uint8Array {
    return this.buf.subarray(0, this.off);
  }
}

// One reusable writer: encoding is synchronous and non-reentrant, and the
// bytes are copied into the command buffer before the next encode begins.
const nodeScratch = new NodeWriter();

/**
 * Encode a plugin-built node tree into the compact binary node format.
 *
 * Reused nodes (those still carrying an arena id, per `reusedId`) at any depth
 * below the root become `{ _ref: id }` placeholders so the Rust rebuild splices
 * the original subtree back in, preserving its id and any patch queued on it.
 * The root is never reffed — it is the new shape replacing the visited node.
 *
 * The format is kind-agnostic: whether the tree is MDAST or HAST is determined
 * on the Rust side by which command applier consumes it.
 */
export function encodeNodeTree(node: unknown, reusedId: ReusedId): Uint8Array {
  nodeScratch.reset();
  encodeNode(nodeScratch, node, true, reusedId);
  return nodeScratch.take();
}

function encodeNode(w: NodeWriter, node: unknown, isRoot: boolean, reusedId: ReusedId): void {
  if (!isRoot && node !== null && typeof node === "object") {
    const id = reusedId(node);
    if (id !== undefined) {
      // Reference placeholder: zero-length type marks a `{ _ref: id }`.
      w.varint(0);
      w.varint(id);
      return;
    }
  }

  const n = node as Record<string, unknown>;
  w.str(typeof n.type === "string" ? n.type : "");

  let mask = 0;
  if (typeof n.value === "string") mask |= 1 << F_VALUE;
  if (typeof n.depth === "number") mask |= 1 << F_DEPTH;
  if (typeof n.url === "string") mask |= 1 << F_URL;
  if (typeof n.title === "string") mask |= 1 << F_TITLE;
  if (typeof n.alt === "string") mask |= 1 << F_ALT;
  if (typeof n.lang === "string") mask |= 1 << F_LANG;
  if (typeof n.meta === "string") mask |= 1 << F_META;
  if (typeof n.ordered === "boolean") mask |= 1 << F_ORDERED;
  if (typeof n.start === "number") mask |= 1 << F_START;
  if (typeof n.spread === "boolean") mask |= 1 << F_SPREAD;
  if (typeof n.checked === "boolean") mask |= 1 << F_CHECKED;
  if (typeof n.identifier === "string") mask |= 1 << F_IDENTIFIER;
  if (typeof n.label === "string") mask |= 1 << F_LABEL;
  if (typeof n.referenceType === "string") mask |= 1 << F_REFERENCE_TYPE;
  if (typeof n.name === "string") mask |= 1 << F_NAME;
  if (typeof n.tagName === "string") mask |= 1 << F_TAG_NAME;
  if (n.attributes != null) mask |= 1 << F_ATTRIBUTES;
  if (n.properties != null) mask |= 1 << F_PROPERTIES;
  if (n.data != null) mask |= 1 << F_DATA;
  if (n._keepChildren === true) mask |= 1 << F_KEEP_CHILDREN;
  w.varint(mask);

  if (mask & (1 << F_VALUE)) w.str(n.value as string);
  if (mask & (1 << F_DEPTH)) w.u8(n.depth as number);
  if (mask & (1 << F_URL)) w.str(n.url as string);
  if (mask & (1 << F_TITLE)) w.str(n.title as string);
  if (mask & (1 << F_ALT)) w.str(n.alt as string);
  if (mask & (1 << F_LANG)) w.str(n.lang as string);
  if (mask & (1 << F_META)) w.str(n.meta as string);
  if (mask & (1 << F_ORDERED)) w.u8(n.ordered ? 1 : 0);
  if (mask & (1 << F_START)) w.varint(n.start as number);
  if (mask & (1 << F_SPREAD)) w.u8(n.spread ? 1 : 0);
  if (mask & (1 << F_CHECKED)) w.u8(n.checked ? 1 : 0);
  if (mask & (1 << F_IDENTIFIER)) w.str(n.identifier as string);
  if (mask & (1 << F_LABEL)) w.str(n.label as string);
  if (mask & (1 << F_REFERENCE_TYPE)) w.str(n.referenceType as string);
  if (mask & (1 << F_NAME)) w.str(n.name as string);
  if (mask & (1 << F_TAG_NAME)) w.str(n.tagName as string);
  if (mask & (1 << F_ATTRIBUTES)) encodeAttributes(w, n.attributes);
  if (mask & (1 << F_PROPERTIES)) encodeProperties(w, n.properties as Record<string, unknown>);
  if (mask & (1 << F_DATA)) w.bytes(encoder.encode(JSON.stringify(n.data)));
  // F_KEEP_CHILDREN carries no payload — the bit alone signals it.

  const children = n.children;
  if (Array.isArray(children) && children.length > 0) {
    w.varint(children.length);
    for (const child of children) encodeNode(w, child, false, reusedId);
  } else {
    w.varint(0);
  }
}

/** Attribute sub-format: `[kind: u8 (0=jsx, 1=directive)][count][entries…]`. */
function encodeAttributes(w: NodeWriter, attrs: unknown): void {
  if (Array.isArray(attrs)) {
    w.u8(0);
    w.varint(attrs.length);
    for (const attr of attrs as Record<string, unknown>[]) {
      if (attr.type === "mdxJsxExpressionAttribute") {
        w.u8(3); // spread
        w.str(typeof attr.value === "string" ? attr.value : String(attr.value ?? ""));
        continue;
      }
      const name = typeof attr.name === "string" ? attr.name : "";
      const v = attr.value;
      if (v === null || v === undefined) {
        w.u8(0); // boolean attribute
        w.str(name);
      } else if (typeof v === "string") {
        w.u8(1); // literal
        w.str(name);
        w.str(v);
      } else if (typeof v === "object" && typeof (v as Record<string, unknown>).value === "string") {
        w.u8(2); // expression
        w.str(name);
        w.str((v as Record<string, unknown>).value as string);
      } else {
        w.u8(0); // fallback: boolean attribute
        w.str(name);
      }
    }
    return;
  }
  if (attrs && typeof attrs === "object") {
    w.u8(1);
    // Directive attribute values are strings; non-strings are dropped by Rust.
    const entries = Object.entries(attrs as Record<string, unknown>).filter(
      ([, v]) => typeof v === "string",
    );
    w.varint(entries.length);
    for (const [k, v] of entries) {
      w.str(k);
      w.str(v as string);
    }
    return;
  }
  w.u8(0);
  w.varint(0);
}

/** Property sub-format: `[count]` then per entry `[name][value_kind: u8][value]`. */
function encodeProperties(w: NodeWriter, props: Record<string, unknown>): void {
  const entries = Object.entries(props).filter(
    ([, v]) =>
      v === null ||
      v === undefined ||
      typeof v === "boolean" ||
      typeof v === "number" ||
      typeof v === "string" ||
      Array.isArray(v),
  );
  w.varint(entries.length);
  for (const [k, v] of entries) {
    w.str(k);
    if (v === true) {
      w.u8(1);
    } else if (v === false) {
      w.u8(2);
    } else if (v === null || v === undefined) {
      w.u8(5);
    } else if (typeof v === "number") {
      w.u8(3);
      w.str(String(v));
    } else if (Array.isArray(v)) {
      w.u8(4);
      const strs = (v as unknown[]).filter((x): x is string => typeof x === "string");
      w.varint(strs.length);
      for (const s of strs) w.str(s);
    } else {
      w.u8(0);
      w.str(v as string);
    }
  }
}

/** A plugin-built node tree, or a `{ raw }` / `{ rawHtml }` wrapper for Rust to
 *  re-parse. Kind-agnostic: MDAST vs HAST is decided by the consuming applier. */
type StructuralNode = object;

export class CommandBuffer {
  private buffer: ArrayBuffer;
  private view: DataView;
  private bytes: Uint8Array;
  private offset: number = 0;
  readonly #reusedId: ReusedId;

  constructor(reusedId: ReusedId = () => undefined) {
    this.#reusedId = reusedId;
    this.buffer = new ArrayBuffer(INITIAL_SIZE);
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
  }

  removeNode(nodeId: number): void {
    this.ensureCapacity(5);
    this.writeU8(CMD_REMOVE);
    this.writeU32(nodeId);
  }

  /** Unified set-property for both MDAST and HAST nodes. */
  setProperty(nodeId: number, key: string, value: unknown): void {
    const encodedName = encoder.encode(key);
    let valueType: number;
    let encodedValue: Uint8Array;

    if (value === null || value === undefined) {
      valueType = PROP_NULL;
      encodedValue = EMPTY_U8;
    } else if (value === true) {
      valueType = PROP_BOOL_TRUE;
      encodedValue = EMPTY_U8;
    } else if (value === false) {
      valueType = PROP_BOOL_FALSE;
      encodedValue = EMPTY_U8;
    } else if (typeof value === "number") {
      valueType = PROP_INT;
      encodedValue = encoder.encode(String(value));
    } else if (Array.isArray(value)) {
      valueType = PROP_SPACE_SEP;
      encodedValue = encoder.encode((value as string[]).join(" "));
    } else {
      valueType = PROP_STRING;
      encodedValue = encoder.encode(String(value));
    }

    // 1(cmd) + 4(nodeId) + 1(valueType) + 4(nameLen) + name + 4(valueLen) + value
    this.ensureCapacity(14 + encodedName.length + encodedValue.length);
    this.writeU8(CMD_SET_PROPERTY);
    this.writeU32(nodeId);
    this.writeU8(valueType);
    this.writeU32(encodedName.length);
    this.writeBytes(encodedName);
    this.writeU32(encodedValue.length);
    this.writeBytes(encodedValue);
  }

  insertBefore(nodeId: number, newNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_INSERT_BEFORE, nodeId, newNode);
  }

  insertAfter(nodeId: number, newNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_INSERT_AFTER, nodeId, newNode);
  }

  prependChild(nodeId: number, newNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_PREPEND_CHILD, nodeId, newNode);
  }

  appendChild(nodeId: number, newNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_APPEND_CHILD, nodeId, newNode);
  }

  wrapNode(nodeId: number, parentNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_WRAP, nodeId, parentNode);
  }

  replace(nodeId: number, newNode: StructuralNode): void {
    this.writeStructuralCommand(CMD_REPLACE, nodeId, newNode);
  }

  /** Return a Uint8Array view of the written bytes (no copy). */
  getBuffer(): Uint8Array {
    return new Uint8Array(this.buffer, 0, this.offset);
  }

  /** Number of bytes written so far. */
  get length(): number {
    return this.offset;
  }

  /** Reset the buffer for reuse, releasing the old ArrayBuffer. */
  reset(): void {
    this.buffer = new ArrayBuffer(INITIAL_SIZE);
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
    this.offset = 0;
  }

  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    const v = node as Record<string, unknown>;
    if (typeof v.raw === "string") {
      this.writeRawPayload(cmd, nodeId, PAYLOAD_RAW_MARKDOWN, encoder.encode(v.raw));
    } else if (typeof v.rawHtml === "string") {
      this.writeRawPayload(cmd, nodeId, PAYLOAD_RAW_HTML, encoder.encode(v.rawHtml));
    } else {
      // Plugin-built node tree, encoded as a compact binary payload.
      this.writeRawPayload(cmd, nodeId, PAYLOAD_BINARY_NODE, encodeNodeTree(node, this.#reusedId));
    }
  }

  private writeRawPayload(
    cmd: number,
    nodeId: number,
    payloadType: number,
    payload: Uint8Array,
  ): void {
    // 1(cmd) + 4(nodeId) + 1(payloadType) + 4(len) + payload
    this.ensureCapacity(10 + payload.length);
    this.writeU8(cmd);
    this.writeU32(nodeId);
    this.writeU8(payloadType);
    this.writeU32(payload.length);
    this.writeBytes(payload);
  }

  private ensureCapacity(needed: number): void {
    while (this.offset + needed > this.buffer.byteLength) {
      this.grow();
    }
  }

  private grow(): void {
    const newBuffer = new ArrayBuffer(this.buffer.byteLength * 2);
    new Uint8Array(newBuffer).set(this.bytes);
    this.buffer = newBuffer;
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
  }

  private writeU8(val: number): void {
    this.view.setUint8(this.offset, val);
    this.offset += 1;
  }

  private writeU32(val: number): void {
    this.view.setUint32(this.offset, val, true);
    this.offset += 4;
  }

  private writeBytes(data: Uint8Array): void {
    this.bytes.set(data, this.offset);
    this.offset += data.length;
  }
}
